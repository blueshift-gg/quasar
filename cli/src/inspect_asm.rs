use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        style, utils,
    },
    std::{
        cmp::Ordering,
        path::PathBuf,
        process::{Command, Stdio},
    },
};

pub fn run(elf_path: Option<PathBuf>, function: Option<String>, source: bool) -> CliResult {
    eprintln!("  {}", style::dim("Preview tool: assembly inspection"));
    let so_path = match elf_path {
        Some(p) => p,
        None => find_so()?,
    };

    if !so_path.exists() {
        return Err(CliError::message(format!(
            "file not found: {}",
            so_path.display()
        )));
    }

    let Some(objdump) = find_objdump() else {
        return Err(CliError::message(
            "llvm-objdump not found in Solana platform-tools.\n\n  Looked in \
             ~/.cache/solana/*/platform-tools/llvm/bin/\n  Install platform-tools: solana-install \
             init",
        ));
    };

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

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            if lines.is_empty() || (function.is_some() && lines.len() <= 2) {
                if let Some(sym) = function {
                    return Err(CliError::message(format!(
                        "symbol not found: {sym}\n  Try a mangled or partial name, e.g. \
                         'entrypoint'"
                    )));
                } else {
                    return Err(CliError::message("no disassembly output"));
                }
            }

            for line in &lines {
                println!("{line}");
            }

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
            let message = if stderr.trim().is_empty() {
                "llvm-objdump failed".to_string()
            } else {
                format!("llvm-objdump failed\n{}", stderr.trim())
            };
            Err(CliError::process_failure(
                message,
                o.status.code().unwrap_or(1),
            ))
        }
        Err(e) => Err(CliError::message(format!(
            "failed to run {}: {e}",
            objdump.display()
        ))),
    }
}

fn find_so() -> Result<PathBuf, crate::error::CliError> {
    let config = QuasarConfig::load()?;
    match utils::find_so(&config, true) {
        Some(p) => Ok(p),
        None => Err(CliError::message(
            "no .so found in target/deploy/ or target/profile/\n  Run `quasar build` first or \
             pass a path: `quasar inspect asm <path>`",
        )),
    }
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
            let version = parse_toolchain_version(name)?;
            let objdump = path.join("platform-tools/llvm/bin/llvm-objdump");
            if objdump.exists() {
                Some((version, objdump))
            } else {
                None
            }
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().next().map(|(_, path)| path)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ToolchainVersion(Vec<u64>);

impl Ord for ToolchainVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let max_len = self.0.len().max(other.0.len());
        for idx in 0..max_len {
            let lhs = self.0.get(idx).copied().unwrap_or(0);
            let rhs = other.0.get(idx).copied().unwrap_or(0);
            match lhs.cmp(&rhs) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }

        Ordering::Equal
    }
}

impl PartialOrd for ToolchainVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_toolchain_version(name: &str) -> Option<ToolchainVersion> {
    let version = name.strip_prefix('v')?;
    let parts = version
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;

    if parts.is_empty() {
        None
    } else {
        Some(ToolchainVersion(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_toolchain_version, ToolchainVersion};

    #[test]
    fn parses_semver_style_versions() {
        assert_eq!(
            parse_toolchain_version("v1.18.22"),
            Some(ToolchainVersion(vec![1, 18, 22]))
        );
        assert_eq!(
            parse_toolchain_version("v2.0"),
            Some(ToolchainVersion(vec![2, 0]))
        );
        assert_eq!(parse_toolchain_version("1.18.22"), None);
        assert_eq!(parse_toolchain_version("v1.18.beta"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        let mut versions = [
            parse_toolchain_version("v1.9.0").expect("parse v1.9.0"),
            parse_toolchain_version("v1.18.22").expect("parse v1.18.22"),
            parse_toolchain_version("v1.10.3").expect("parse v1.10.3"),
            parse_toolchain_version("v2.0.0").expect("parse v2.0.0"),
        ];

        versions.sort();

        assert_eq!(
            versions,
            [
                ToolchainVersion(vec![1, 9, 0]),
                ToolchainVersion(vec![1, 10, 3]),
                ToolchainVersion(vec![1, 18, 22]),
                ToolchainVersion(vec![2, 0, 0]),
            ]
        );
    }
}
