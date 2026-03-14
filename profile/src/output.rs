use {
    crate::aggregate::ProfileResult,
    serde::Serialize,
    std::{
        collections::HashMap,
        io::{BufWriter, Write},
        mem,
        path::Path,
    },
};

pub fn print_summary(result: &ProfileResult) {
    eprintln!("Total .text instructions: {} CUs", result.total_cus);
    eprintln!();
    eprintln!("Top functions by CU (leaf attribution):");

    let top_n = 20.min(result.function_cus.len());
    for (i, (name, cus)) in result.function_cus.iter().take(top_n).enumerate() {
        let pct = *cus as f64 / result.total_cus as f64 * 100.0;
        eprintln!("  {:>3}. {:>6} CUs ({:>5.1}%)  {}", i + 1, cus, pct, name);
    }

    if result.function_cus.len() > top_n {
        eprintln!(
            "  ... and {} more functions",
            result.function_cus.len() - top_n
        );
    }

    eprintln!();
    eprintln!("Note: Syscall CU costs (CPI, logging, etc.) are runtime-dependent and excluded.");
}

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
