use {
    crate::{config::QuasarConfig, error::CliResult, style, toolchain},
    std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::Instant,
    },
};

extern crate toml;

pub fn run(debug: bool, watch: bool) -> CliResult {
    if watch {
        return run_watch(debug);
    }

    run_once(debug)
}

fn run_once(debug: bool) -> CliResult {
    let config = QuasarConfig::load()?;
    let start = Instant::now();

    crate::idl::generate(Path::new("."), config.has_typescript_tests())?;

    let sp = style::spinner("Building...");

    let output = if config.is_solana_toolchain() {
        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf");
        if debug {
            cmd.arg("--debug");
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    } else {
        if !toolchain::has_sbpf_linker() {
            sp.finish_and_clear();
            eprintln!("\n  {}", style::fail("sbpf-linker not found on PATH."));
            eprintln!();
            eprintln!("  Install platform-tools first:");
            eprintln!(
                "    {}",
                style::bold("git clone https://github.com/anza-xyz/platform-tools")
            );
            eprintln!("    {}", style::bold("cd platform-tools"));
            eprintln!("    {}", style::bold("cargo install-with-gallery"));
            std::process::exit(1);
        }

        let mut cmd = Command::new("cargo");
        if debug {
            cmd.env("RUSTFLAGS", "-C link-arg=--btf -C debuginfo=2");
        }
        cmd.arg("build-bpf");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    };

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let elapsed = start.elapsed();

            if !config.is_solana_toolchain() {
                let program = config.module_name();
                let src = PathBuf::from("target")
                    .join("bpfel-unknown-none")
                    .join("release")
                    .join(format!("lib{}.so", program));
                let dest_dir = PathBuf::from("target").join("deploy");
                fs::create_dir_all(&dest_dir)?;
                let dest = dest_dir.join(format!("lib{}.so", program));
                fs::copy(&src, &dest).map_err(|e| {
                    eprintln!(
                        "  {}",
                        style::fail(&format!("failed to copy {}: {e}", src.display()))
                    );
                    e
                })?;
            }

            // Show warnings even on success
            let stderr = String::from_utf8_lossy(&o.stderr);
            let warnings = extract_warnings(&stderr);
            if !warnings.is_empty() {
                eprintln!();
                for line in &warnings {
                    eprintln!("  {line}");
                }
            }

            let so_path = find_so(&config);
            let size_info = so_path
                .and_then(|p| {
                    let meta = fs::metadata(&p).ok()?;
                    let new_size = meta.len();
                    let delta = size_delta(&p, new_size);
                    save_last_size(&p, new_size);
                    Some(format!(
                        " ({}{delta})",
                        style::dim(&style::human_size(new_size))
                    ))
                })
                .unwrap_or_default();

            println!(
                "  {}",
                style::success(&format!(
                    "Build complete in {}{size_info}",
                    style::bold(&style::human_duration(elapsed))
                ))
            );
            Ok(())
        }
        Ok(o) => {
            let elapsed = start.elapsed();
            let stderr = String::from_utf8_lossy(&o.stderr);
            print_build_errors(&stderr, elapsed);
            std::process::exit(o.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "  {}",
                style::fail(&format!("failed to run build command: {e}"))
            );
            std::process::exit(1);
        }
    }
}

/// Build with debug symbols only (no feature flags) for profiling.
/// Copies the .so to target/profile/ and returns the path.
pub fn profile_build() -> Result<PathBuf, crate::error::CliError> {
    let config = QuasarConfig::load()?;
    let start = Instant::now();

    crate::idl::generate(Path::new("."), config.has_typescript_tests())?;

    let sp = style::spinner("Profile build...");

    let output = if config.is_solana_toolchain() {
        let mut cmd = Command::new("cargo");
        cmd.arg("build-sbf").arg("--debug");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    } else {
        if !toolchain::has_sbpf_linker() {
            sp.finish_and_clear();
            eprintln!("\n  {}", style::fail("sbpf-linker not found on PATH."));
            eprintln!();
            eprintln!("  Install platform-tools first:");
            eprintln!(
                "    {}",
                style::bold("git clone https://github.com/anza-xyz/platform-tools")
            );
            eprintln!("    {}", style::bold("cd platform-tools"));
            eprintln!("    {}", style::bold("cargo install-with-gallery"));
            std::process::exit(1);
        }

        // Read existing rustflags from .cargo/config.toml and append debug flags
        let existing_flags = read_target_rustflags();
        let mut all_flags = existing_flags;
        all_flags.extend([
            "-C".to_string(),
            "link-arg=--btf".to_string(),
            "-C".to_string(),
            "debuginfo=2".to_string(),
        ]);

        // Use CARGO_ENCODED_RUSTFLAGS (0x1f-separated) which takes priority
        let encoded = all_flags.join("\x1f");
        let mut cmd = Command::new("cargo");
        cmd.env("CARGO_ENCODED_RUSTFLAGS", encoded);
        cmd.arg("build-bpf");
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()
    };

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let elapsed = start.elapsed();
            let program = config.module_name();
            let profile_dir = PathBuf::from("target").join("profile");
            fs::create_dir_all(&profile_dir)?;

            // Find the built .so and copy to target/profile/
            let src = if config.is_solana_toolchain() {
                // build-sbf --debug puts it in target/deploy/ or
                // target/sbf-solana-solana/release/
                find_so(&config).unwrap_or_else(|| {
                    PathBuf::from("target")
                        .join("sbf-solana-solana")
                        .join("release")
                        .join(format!("{}.so", program))
                })
            } else {
                PathBuf::from("target")
                    .join("bpfel-unknown-none")
                    .join("release")
                    .join(format!("lib{}.so", program))
            };

            let dest = profile_dir.join(format!("{}.so", program));
            fs::copy(&src, &dest).map_err(|e| {
                eprintln!(
                    "  {}",
                    style::fail(&format!("failed to copy {}: {e}", src.display()))
                );
                e
            })?;

            let size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            println!(
                "  {}",
                style::success(&format!(
                    "Profile build in {} ({})",
                    style::bold(&style::human_duration(elapsed)),
                    style::dim(&style::human_size(size))
                ))
            );

            Ok(dest)
        }
        Ok(o) => {
            let elapsed = start.elapsed();
            let stderr = String::from_utf8_lossy(&o.stderr);
            print_build_errors(&stderr, elapsed);
            std::process::exit(o.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "  {}",
                style::fail(&format!("failed to run build command: {e}"))
            );
            std::process::exit(1);
        }
    }
}

fn find_so(config: &QuasarConfig) -> Option<PathBuf> {
    let module = config.module_name();
    let name = &config.project.name;
    [
        format!("target/deploy/{name}.so"),
        format!("target/deploy/{module}.so"),
        format!("target/deploy/lib{module}.so"),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

fn run_watch(debug: bool) -> CliResult {
    if let Err(e) = run_once(debug) {
        eprintln!("  {}", style::fail(&format!("{e}")));
    }

    loop {
        let baseline = collect_mtimes(Path::new("src"));
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let current = collect_mtimes(Path::new("src"));
            if current != baseline {
                if let Err(e) = run_once(debug) {
                    eprintln!("  {}", style::fail(&format!("{e}")));
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Build error formatting
// ---------------------------------------------------------------------------

/// Extract warning lines from cargo output (for display on success).
fn extract_warnings(stderr: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut capture = false;

    for line in stderr.lines() {
        if line.starts_with("warning") {
            if line.contains("warnings emitted")
                || line.contains("warning emitted")
                || line.contains("user-defined alias")
                || line.contains("shadowing")
            {
                continue;
            }
            capture = true;
            warnings.push(line.to_string());
        } else if capture {
            if line.starts_with("  ") || line.starts_with(" -->") || line.is_empty() {
                warnings.push(line.to_string());
            } else {
                capture = false;
            }
        }
    }

    warnings
}

/// Extract and display only the meaningful error/warning lines from cargo
/// output.
fn print_build_errors(stderr: &str, elapsed: std::time::Duration) {
    let mut errors: Vec<String> = Vec::new();
    let mut capture = false;

    for line in stderr.lines() {
        // Primary error/warning lines from rustc or cargo
        if line.starts_with("error") || line.starts_with("warning") {
            // Skip "warning: N warnings emitted" summary lines
            if line.contains("warnings emitted") || line.contains("warning emitted") {
                continue;
            }
            // Skip the cargo alias shadow warning
            if line.contains("user-defined alias") || line.contains("shadowing") {
                continue;
            }
            capture = true;
            errors.push(line.to_string());
        } else if capture {
            // Capture continuation lines (source snippets, arrows, notes, "Caused by:")
            if line.starts_with("  ")
                || line.starts_with(" -->")
                || line.starts_with("Caused by:")
                || line.is_empty()
            {
                errors.push(line.to_string());
            } else {
                capture = false;
            }
        }
    }

    if errors.is_empty() {
        // Fallback: show raw stderr if we couldn't parse errors
        if !stderr.is_empty() {
            eprint!("{stderr}");
        }
        eprintln!(
            "  {}",
            style::fail(&format!(
                "build failed in {}",
                style::bold(&style::human_duration(elapsed))
            ))
        );
        return;
    }

    eprintln!();
    for line in &errors {
        eprintln!("  {line}");
    }
    eprintln!();

    // Count errors vs warnings
    let err_count = errors.iter().filter(|l| l.starts_with("error")).count();
    let warn_count = errors.iter().filter(|l| l.starts_with("warning")).count();

    let mut summary = String::new();
    if err_count > 0 {
        summary.push_str(&format!(
            "{err_count} error{}",
            if err_count == 1 { "" } else { "s" }
        ));
    }
    if warn_count > 0 {
        if !summary.is_empty() {
            summary.push_str(", ");
        }
        summary.push_str(&format!(
            "{warn_count} warning{}",
            if warn_count == 1 { "" } else { "s" }
        ));
    }

    eprintln!(
        "  {}",
        style::fail(&format!(
            "build failed in {} ({summary})",
            style::bold(&style::human_duration(elapsed))
        ))
    );
}

// ---------------------------------------------------------------------------
// Build size tracking
// ---------------------------------------------------------------------------

const LAST_SIZE_FILE: &str = "target/.quasar-last-size";

fn size_delta(so_path: &Path, new_size: u64) -> String {
    let key = so_path.to_string_lossy();
    let last = fs::read_to_string(LAST_SIZE_FILE)
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|l| l.starts_with(&*key))
                .and_then(|l| l.rsplit_once(' '))
                .and_then(|(_, s)| s.parse::<u64>().ok())
        });

    let Some(prev) = last else {
        return String::new();
    };

    if new_size == prev {
        return String::new();
    }

    let diff = new_size as i64 - prev as i64;
    if diff > 0 {
        format!(
            ", {}",
            style::color(196, &format!("+{}", style::human_size(diff as u64)))
        )
    } else {
        format!(
            ", {}",
            style::color(83, &format!("-{}", style::human_size((-diff) as u64)))
        )
    }
}

fn save_last_size(so_path: &Path, size: u64) {
    let key = so_path.to_string_lossy();
    let entry = format!("{key} {size}");

    // Read existing entries, replace or append
    let mut lines: Vec<String> = fs::read_to_string(LAST_SIZE_FILE)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(&*key))
        .map(String::from)
        .collect();
    lines.push(entry);
    let _ = fs::write(LAST_SIZE_FILE, lines.join("\n"));
}

/// Read rustflags from .cargo/config.toml for the bpfel-unknown-none target.
fn read_target_rustflags() -> Vec<String> {
    let config_path = Path::new(".cargo").join("config.toml");
    let contents = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let value: toml::Value = match contents.parse() {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    value
        .get("target")
        .and_then(|t| t.get("bpfel-unknown-none"))
        .and_then(|t| t.get("rustflags"))
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn collect_mtimes(dir: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut times = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                times.extend(collect_mtimes(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                if let Ok(meta) = fs::metadata(&path) {
                    if let Ok(mtime) = meta.modified() {
                        times.push((path, mtime));
                    }
                }
            }
        }
    }
    times.sort_by(|a, b| a.0.cmp(&b.0));
    times
}
