use {
    crate::aggregate::ProfileResult,
    serde::Serialize,
    std::{
        collections::HashMap,
        fs,
        io::{BufWriter, Write},
        mem,
        path::Path,
    },
};

// ---------------------------------------------------------------------------
// ANSI helpers (standalone — profile crate doesn't depend on cli)
// ---------------------------------------------------------------------------

fn bold(s: &str) -> String {
    format!("\x1b[1m{s}\x1b[0m")
}

fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[0m")
}

fn cyan(s: &str) -> String {
    format!("\x1b[36m{s}\x1b[0m")
}

fn red(s: &str) -> String {
    format!("\x1b[38;5;196m{s}\x1b[0m")
}

fn green(s: &str) -> String {
    format!("\x1b[38;5;83m{s}\x1b[0m")
}

fn bar_fill(s: &str) -> String {
    cyan(s)
}

// ---------------------------------------------------------------------------
// Terminal profile summary
// ---------------------------------------------------------------------------

const LAST_PROFILE: &str = "target/profile/.last-profile";

pub fn print_summary(result: &ProfileResult, program_name: &str, _binary_size: u64, expand: bool) {
    println!();
    let total = result.total_cus;
    let fn_count = result.function_cus.len();

    // Load previous profile for diffing
    let prev = load_previous_profile(program_name);
    let prev_total = prev.as_ref().map(|p| p.values().sum::<u64>());

    // Header: name + CU + inline delta
    let total_delta = match prev_total {
        Some(pt) => {
            let diff = total as i64 - pt as i64;
            if diff > 0 {
                format!("  {}", red(&format!("+{}", format_cu(diff as u64))))
            } else if diff < 0 {
                format!("  {}", green(&format!("-{}", format_cu((-diff) as u64))))
            } else {
                format!("  {}", dim("="))
            }
        }
        _ => String::new(),
    };

    println!(
        "  {}  {}{}",
        bold(program_name),
        cyan(&format!("{} CU", format_cu(total))),
        total_delta,
    );

    if fn_count == 0 {
        save_current_profile(program_name, result);
        return;
    }

    let has_baseline = prev.is_some();

    if expand {
        print_full_table(result, prev.as_ref(), total, fn_count);
    } else if has_baseline {
        print_deltas(result, prev.as_ref().unwrap(), total);
    } else {
        print_top_functions(result, total);
    }

    save_current_profile(program_name, result);
}

/// First run — show the top hottest functions so the user has a baseline.
fn print_top_functions(result: &ProfileResult, total: u64) {
    let fn_count = result.function_cus.len();
    let show = 5.min(fn_count);

    for (name, cus) in result.function_cus.iter().take(show) {
        let pct = *cus as f64 / total as f64 * 100.0;
        println!(
            "  {:>8} {}  {}",
            format_cu(*cus),
            dim(&format!("{:>5.1}%", pct)),
            simplify_name(name),
        );
    }

    if fn_count > show {
        let rest: u64 = result.function_cus.iter().skip(show).map(|(_, c)| c).sum();
        println!(
            "  {}",
            dim(&format!(
                "{:>8} {:>5.1}%  +{} more (--expand)",
                format_cu(rest),
                rest as f64 / total as f64 * 100.0,
                fn_count - show,
            ))
        );
    }
}

/// Subsequent runs — show the biggest regressions and improvements.
fn print_deltas(result: &ProfileResult, prev: &HashMap<String, u64>, total: u64) {
    let mut deltas: Vec<(&str, i64, u64)> = result
        .function_cus
        .iter()
        .filter_map(|(name, cus)| {
            let prev_cu = prev.get(name).copied().unwrap_or(0);
            let diff = *cus as i64 - prev_cu as i64;
            if diff != 0 {
                Some((name.as_str(), diff, *cus))
            } else {
                None
            }
        })
        .collect();

    // Also check for functions that disappeared (were in prev but not in current)
    for (name, prev_cu) in prev {
        if !result.function_cus.iter().any(|(n, _)| n == name) {
            deltas.push((name.as_str(), -(*prev_cu as i64), 0));
        }
    }

    if deltas.is_empty() {
        return;
    }

    // Sort by absolute magnitude (biggest changes first)
    deltas.sort_by_key(|d| std::cmp::Reverse(d.1.unsigned_abs()));

    let show = 10.min(deltas.len());
    for (name, diff, cus) in deltas.iter().take(show) {
        let cu_str = if *cus > 0 {
            format_cu(*cus)
        } else {
            "removed".to_string()
        };
        let delta_str = if *diff > 0 {
            red(&format!("+{}", format_cu(diff.unsigned_abs())))
        } else {
            green(&format!("-{}", format_cu(diff.unsigned_abs())))
        };
        let pct = if total > 0 {
            diff.unsigned_abs() as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "  {:>8} {}  {}  {}",
            cu_str,
            delta_str,
            dim(&format!("{:>5.1}%", pct)),
            simplify_name(name),
        );
    }

    if deltas.len() > show {
        println!(
            "  {}",
            dim(&format!("+{} more (--expand)", deltas.len() - show))
        );
    }
}

/// --expand: full view with bars and per-function deltas.
fn print_full_table(
    result: &ProfileResult,
    prev: Option<&HashMap<String, u64>>,
    total: u64,
    fn_count: usize,
) {
    let max_cu = result.function_cus.first().map(|(_, c)| *c).unwrap_or(1);
    let bar_width: usize = 20;

    for (name, cus) in &result.function_cus {
        let pct = *cus as f64 / total as f64 * 100.0;
        let filled = (*cus as f64 / max_cu as f64 * bar_width as f64).round() as usize;
        let bar_filled: String = "█".repeat(filled);
        let bar_empty: String = "░".repeat(bar_width - filled);

        let delta = prev
            .and_then(|p| {
                let prev_cu = p.get(name)?;
                let diff = *cus as i64 - *prev_cu as i64;
                if diff == 0 {
                    None
                } else if diff > 0 {
                    Some(red(&format!(" +{diff}")))
                } else {
                    Some(green(&format!(" {diff}")))
                }
            })
            .unwrap_or_default();

        println!(
            "  {:>8} {}  {}{}  {}{}",
            format_cu(*cus),
            dim(&format!("{:>5.1}%", pct)),
            bar_fill(&bar_filled),
            dim(&bar_empty),
            simplify_name(name),
            delta,
        );
    }

    println!(
        "  {}",
        dim(&format!(
            "{fn_count} functions, {} CU total",
            format_cu(total)
        ))
    );
}

pub fn print_flamegraph_link(url: &str) {
    println!("  {}  {}", dim("flamegraph"), cyan(url));
}

fn load_previous_profile(program_name: &str) -> Option<HashMap<String, u64>> {
    let path = format!("{LAST_PROFILE}.{program_name}");
    let contents = fs::read_to_string(path).ok()?;
    let mut map = HashMap::new();
    for line in contents.lines() {
        let (cu_str, name) = line.split_once(' ')?;
        let cu: u64 = cu_str.parse().ok()?;
        map.insert(name.to_string(), cu);
    }
    Some(map)
}

fn save_current_profile(program_name: &str, result: &ProfileResult) {
    let path = format!("{LAST_PROFILE}.{program_name}");
    if let Some(parent) = Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let contents: String = result
        .function_cus
        .iter()
        .map(|(name, cu)| format!("{cu} {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(path, contents);
}

/// Simplify a demangled Rust function name for the terminal.
///
/// Turns `<quasar_test::instructions::initialize::Initialize as
/// quasar_core::Accounts>::verify` into `Initialize::verify`
fn simplify_name(name: &str) -> String {
    let name = name.trim();

    if name == "[unknown]" || name == "entrypoint" {
        return name.to_string();
    }

    // Handle `<Type as Trait>::method` → `Type::method`
    let working = if name.starts_with('<') {
        if let Some(rest) = name.strip_prefix('<') {
            // Find matching '>' for the impl block
            if let Some(as_pos) = rest.find(" as ") {
                let type_part = &rest[..as_pos];
                // Find the closing '>'
                if let Some(gt_pos) = rest[as_pos..].find('>') {
                    let after = &rest[as_pos + gt_pos + 1..];
                    format!("{type_part}{after}")
                } else {
                    type_part.to_string()
                }
            } else if let Some(gt_pos) = rest.find('>') {
                let type_part = &rest[..gt_pos];
                let after = &rest[gt_pos + 1..];
                format!("{type_part}{after}")
            } else {
                name.to_string()
            }
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    };

    // Strip generic parameters <...>
    let mut result = String::new();
    let mut depth = 0i32;
    for ch in working.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }

    // Take the last N meaningful segments
    let segments: Vec<&str> = result.split("::").filter(|s| !s.is_empty()).collect();
    let simplified = if segments.is_empty() {
        return name.to_string();
    } else if segments.len() <= 2 {
        segments.join("::")
    } else {
        // Take last 2, but if the second-to-last is a single char or starts with
        // lowercase (like a module), take 3
        let last2 = &segments[segments.len() - 2..];
        if last2[0].len() <= 1 || last2[0].starts_with(|c: char| c.is_lowercase()) {
            if segments.len() >= 3 {
                segments[segments.len() - 3..].join("::")
            } else {
                last2.join("::")
            }
        } else {
            last2.join("::")
        }
    };

    // If we ended up with just a generic param like "T" or "U", use the method name
    if simplified.is_empty()
        || (simplified.len() == 1 && simplified.chars().next().unwrap().is_uppercase())
    {
        // Grab the last `::segment` from the original name (before generics)
        let last_fn = name
            .rsplit("::")
            .next()
            .unwrap_or(name)
            .trim_end_matches([')', '(', ' ']);
        if last_fn.is_empty() || last_fn == name {
            name.to_string()
        } else {
            last_fn.to_string()
        }
    } else {
        simplified
    }
}

/// Format CU count with comma separators
fn format_cu(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

// ---------------------------------------------------------------------------
// JSON output (unchanged)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ProfileData {
    program: String,
    version: String,
    #[serde(rename = "binaryHash")]
    binary_hash: String,
    #[serde(rename = "binarySize")]
    binary_size: u64,
    root: FrameNode,
}

#[derive(Serialize)]
struct FrameNode {
    name: String,
    value: u64,
    children: Vec<FrameNode>,
}

#[derive(Default)]
struct BuildNode {
    value: u64,
    children: HashMap<String, BuildNode>,
}

pub fn write_json(
    result: &ProfileResult,
    path: &Path,
    program_name: &str,
    version: &str,
    binary_size: u64,
    binary_hash: &str,
) {
    let root = frame_tree_from_folded(&result.folded_stacks, result.total_cus);
    let profile = ProfileData {
        program: program_name.to_string(),
        version: version.to_string(),
        binary_hash: binary_hash.to_string(),
        binary_size,
        root,
    };

    let file = std::fs::File::create(path).unwrap_or_else(|e| {
        eprintln!("Error: failed to create {}: {}", path.display(), e);
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &profile).unwrap_or_else(|e| {
        eprintln!("Error: failed to serialize JSON profile: {}", e);
        std::process::exit(1);
    });
    writer.write_all(b"\n").unwrap();
    writer.flush().unwrap();
}

fn frame_tree_from_folded(folded: &str, total: u64) -> FrameNode {
    let mut synthetic = BuildNode::default();

    for line in folded.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((stack, count_str)) = trimmed.rsplit_once(' ') else {
            continue;
        };
        let Ok(count) = count_str.parse::<u64>() else {
            continue;
        };

        let mut cursor = &mut synthetic;
        for part in stack.split(';') {
            let node = cursor.children.entry(part.to_string()).or_default();
            node.value += count;
            cursor = node;
        }
    }

    if synthetic.children.len() == 1 {
        let (name, node) = synthetic.children.into_iter().next().unwrap();
        return to_frame_node(name, node);
    }

    let mut children: Vec<FrameNode> = synthetic
        .children
        .into_iter()
        .map(|(name, node)| to_frame_node(name, node))
        .collect();
    children.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name)));
    FrameNode {
        name: "all".to_string(),
        value: total,
        children,
    }
}

fn to_frame_node(name: String, mut node: BuildNode) -> FrameNode {
    let children_map = mem::take(&mut node.children);
    let mut children: Vec<FrameNode> = children_map
        .into_iter()
        .map(|(child_name, child)| to_frame_node(child_name, child))
        .collect();
    children.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name)));
    FrameNode {
        name,
        value: node.value,
        children,
    }
}
