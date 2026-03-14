use {
    crate::{config::QuasarConfig, error::CliResult, style},
    std::{
        path::PathBuf,
        process::{Command, Stdio},
    },
};

pub fn run(elf_path: Option<PathBuf>, function: Option<String>, source: bool) -> CliResult {
    let so_path = match elf_path {
        Some(p) => p,
        None => find_so()?,
    };

    if !so_path.exists() {
        eprintln!(
            "  {}",
            style::fail(&format!("file not found: {}", so_path.display()))
        );
        std::process::exit(1);
    }

    let objdump = find_objdump().unwrap_or_else(|| {
        eprintln!(
            "  {}",
            style::fail("llvm-objdump not found in Solana platform-tools.")
        );
        eprintln!();
        eprintln!("  Looked in ~/.cache/solana/*/platform-tools/llvm/bin/");
        eprintln!(
            "  Install platform-tools: {}",
            style::bold("solana-install init")
        );
        std::process::exit(1);
    });

    let mut cmd = Command::new(&objdump);
    cmd.arg("-d") // disassemble
        .arg("-C") // demangle
        .arg("--no-show-raw-insn"); // cleaner output

    if source {
        cmd.arg("-S"); // interleave source
    }

    if let Some(ref sym) = function {
        cmd.arg(format!("--disassemble-symbols={sym}"));
    }

    cmd.arg(&so_path);

    // If piping to a pager, let it handle output directly
    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            if lines.is_empty() || (function.is_some() && lines.len() <= 2) {
                if let Some(sym) = function {
                    eprintln!("  {}", style::fail(&format!("symbol not found: {sym}")));
                    eprintln!(
                        "  {}",
                        style::dim("Try a mangled or partial name, e.g. 'entrypoint'")
                    );
                } else {
                    eprintln!("  {}", style::fail("no disassembly output"));
                }
                std::process::exit(1);
            }

            // Print with minimal framing
            for line in &lines {
                println!("{line}");
            }

            // Summary
            let insn_count = lines
                .iter()
                .filter(|l| {
                    let trimmed = l.trim();
                    // Instruction lines start with an address (hex digits followed by colon)
                    trimmed.split(':').next().is_some_and(|addr| {
                        !addr.is_empty() && addr.trim().chars().all(|c| c.is_ascii_hexdigit())
                    })
                })
                .count();

            eprintln!(
                "\n  {} {} instructions ({})",
                style::dim("sBPF"),
                insn_count,
                style::dim(&so_path.display().to_string()),
            );

            Ok(())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("  {}", style::fail("llvm-objdump failed"));
            if !stderr.trim().is_empty() {
                eprintln!("  {}", stderr.trim());
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!(
                "  {}",
                style::fail(&format!("failed to run {}: {e}", objdump.display()))
            );
            std::process::exit(1);
        }
    }
}

/// Find the .so in target/deploy/ or target/profile/
fn find_so() -> Result<PathBuf, crate::error::CliError> {
    let config = QuasarConfig::load()?;
    let module = config.module_name();
    let name = &config.project.name;

    let candidates = [
        format!("target/deploy/{name}.so"),
        format!("target/deploy/{module}.so"),
        format!("target/deploy/lib{module}.so"),
        format!("target/profile/{module}.so"),
    ];

    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
    }

    eprintln!(
        "  {}",
        style::fail("no .so found in target/deploy/ or target/profile/")
    );
    eprintln!(
        "  {}",
        style::dim("Run `quasar build` first or pass a path: `quasar dump <path>`")
    );
    std::process::exit(1);
}

/// Find llvm-objdump in Solana platform-tools (newest version first)
fn find_objdump() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let cache = home.join(".cache/solana");
    if !cache.exists() {
        return None;
    }

    let mut versions: Vec<_> = std::fs::read_dir(&cache)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?;
            let ver = name.strip_prefix('v')?;
            let num: f64 = ver.parse().ok()?;
            let objdump = path.join("platform-tools/llvm/bin/llvm-objdump");
            if objdump.exists() {
                Some((num, objdump))
            } else {
                None
            }
        })
        .collect();
    versions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    versions.into_iter().next().map(|(_, path)| path)
}
