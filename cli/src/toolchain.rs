use {
    crate::style,
    std::{
        path::Path,
        process::Command,
    },
};

// ---------------------------------------------------------------------------
// Compatibility matrix
// ---------------------------------------------------------------------------

/// Required toolchain versions for a given quasar-lang version.
pub struct ToolchainRequirements {
    pub solana_version: &'static str,
    pub rust_version: &'static str,
}

/// Compatibility matrix: maps quasar-lang versions to required toolchain versions.
/// Updated with each CLI release.
///
/// `solana_version` is a concrete installable version (e.g. "2.1.21").
/// Comparison uses major.minor only — any 2.1.x satisfies a "2.1.21" requirement.
/// `rust_version` is a minimum version (e.g. "1.87.0" means >= 1.87.0).
const COMPAT_TABLE: &[(&str, ToolchainRequirements)] = &[
    ("0.0.0", ToolchainRequirements { solana_version: "3.0.0", rust_version: "1.87.0" }),
    // ("0.1.0", ToolchainRequirements { solana_version: "3.1.0", rust_version: "1.87.0" }),
];

/// The latest quasar-lang version this CLI knows about.
/// Used by `quasar init` to pin the framework version in new projects.
pub const LATEST_KNOWN_VERSION: &str = "0.0.0";

/// Look up toolchain requirements for a quasar-lang version.
/// Returns None if the version is unknown (CLI needs updating).
pub fn requirements_for(quasar_lang_version: &str) -> Option<&'static ToolchainRequirements> {
    COMPAT_TABLE
        .iter()
        .find(|(v, _)| *v == quasar_lang_version)
        .map(|(_, req)| req)
}

// ---------------------------------------------------------------------------
// Version detection
// ---------------------------------------------------------------------------

/// Read the quasar-lang version from a project's Cargo.toml.
/// Looks for `quasar-lang = "X.Y.Z"` or `quasar-lang = { version = "X.Y.Z", ... }`.
/// Falls back to Cargo.lock if Cargo.toml doesn't have a pinned version (e.g. git dep).
pub fn detect_quasar_lang_version(project_root: &Path) -> Option<String> {
    if let Some(v) = read_version_from_cargo_toml(project_root) {
        return Some(v);
    }
    read_version_from_cargo_lock(project_root)
}

fn read_version_from_cargo_toml(project_root: &Path) -> Option<String> {
    let toml_path = project_root.join("Cargo.toml");
    let contents = std::fs::read_to_string(&toml_path).ok()?;
    let parsed: toml::Value = contents.parse().ok()?;

    let deps = parsed.get("dependencies")?;
    let quasar = deps.get("quasar-lang")?;

    match quasar {
        toml::Value::String(v) => Some(v.clone()),
        toml::Value::Table(t) => t.get("version")?.as_str().map(String::from),
        _ => None,
    }
}

fn read_version_from_cargo_lock(project_root: &Path) -> Option<String> {
    let lock_path = project_root.join("Cargo.lock");
    let contents = std::fs::read_to_string(&lock_path).ok()?;
    let lock: toml::Value = contents.parse().ok()?;
    let packages = lock.get("package")?.as_array()?;

    packages.iter().find_map(|pkg| {
        let name = pkg.get("name")?.as_str()?;
        if name == "quasar-lang" {
            pkg.get("version")?.as_str().map(String::from)
        } else {
            None
        }
    })
}

/// Get the installed Solana CLI version (e.g. "2.1.6").
pub fn installed_solana_version() -> Option<String> {
    let output = Command::new("solana")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .nth(1)
        .map(|v| v.trim().to_string())
}

/// Get the installed Rust compiler version (e.g. "1.87.0").
pub fn installed_rust_version() -> Option<String> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .nth(1)
        .map(|v| v.split('-').next().unwrap_or(v).to_string())
}

// ---------------------------------------------------------------------------
// Toolchain auto-switch
// ---------------------------------------------------------------------------

/// Ensure the project's toolchain requirements are met.
///
/// - No version found: silently skips.
/// - Unknown version: warns and proceeds (CLI needs updating).
/// - Solana major.minor mismatch: auto-installs correct version.
/// - Rust below minimum: warns (user manages via rustup).
pub fn ensure_toolchain(project_root: &Path) {
    let version = match detect_quasar_lang_version(project_root) {
        Some(v) => v,
        None => return,
    };

    let reqs = match requirements_for(&version) {
        Some(r) => r,
        None => {
            eprintln!();
            eprintln!(
                "  {} quasar-lang v{version} is not recognized by this CLI version.",
                style::warn(""),
            );
            eprintln!(
                "    Could not auto-switch Solana and Rust versions."
            );
            eprintln!(
                "    Run {} to get the latest toolchain mappings.",
                style::bold("quasar update")
            );
            eprintln!();
            return;
        }
    };

    match installed_solana_version() {
        Some(ref installed) if major_minor(installed) == major_minor(reqs.solana_version) => {}
        Some(ref installed) => {
            eprintln!();
            eprintln!(
                "  {} Switching Solana CLI: v{installed} -> v{} (required by quasar-lang v{version})",
                style::dim(""),
                reqs.solana_version,
            );
            install_solana(reqs.solana_version);
        }
        None => {
            eprintln!();
            eprintln!(
                "  {} Installing Solana CLI v{} (required by quasar-lang v{version})...",
                style::dim(""),
                reqs.solana_version,
            );
            install_solana(reqs.solana_version);
        }
    }

    if let Some(ref installed) = installed_rust_version() {
        if version_less_than(installed, reqs.rust_version) {
            eprintln!();
            eprintln!(
                "  {} Rust v{installed} found, but quasar-lang v{version} requires >= v{}.",
                style::warn(""),
                reqs.rust_version,
            );
            eprintln!(
                "    Run {} to update.",
                style::bold(&format!("rustup install {}", reqs.rust_version))
            );
            eprintln!();
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether sbpf-linker is reachable on PATH.
pub fn has_sbpf_linker() -> bool {
    Command::new("sbpf-linker")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

fn major_minor(v: &str) -> String {
    let mut parts = v.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("{major}.{minor}")
}

fn version_less_than(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let mut parts = v.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    parse(a) < parse(b)
}

fn install_solana(version: &str) {
    let result = Command::new("agave-install")
        .args(["init", version])
        .status();

    match result {
        Ok(status) if status.success() => {
            eprintln!(
                "  {}",
                style::success(&format!("Solana CLI v{version} ready."))
            );
        }
        _ => {
            let result = Command::new("solana-install")
                .args(["init", version])
                .status();

            match result {
                Ok(status) if status.success() => {
                    eprintln!(
                        "  {}",
                        style::success(&format!("Solana CLI v{version} ready."))
                    );
                }
                _ => {
                    eprintln!();
                    eprintln!(
                        "  {}",
                        style::fail(&format!("Failed to install Solana CLI v{version}."))
                    );
                    eprintln!();
                    eprintln!(
                        "  Install manually: {}",
                        style::bold(&format!(
                            "sh -c \"$(curl -sSfL https://release.anza.xyz/v{version}/install)\""
                        ))
                    );
                    eprintln!();
                    std::process::exit(1);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_version_from_cargo_toml_string() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
quasar-lang = "0.2.0"
"#;
        std::fs::write(dir.path().join("Cargo.toml"), content).unwrap();
        assert_eq!(
            detect_quasar_lang_version(dir.path()),
            Some("0.2.0".to_string())
        );
    }

    #[test]
    fn detect_version_from_cargo_toml_table() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
quasar-lang = { version = "0.1.0", features = ["alloc"] }
"#;
        std::fs::write(dir.path().join("Cargo.toml"), content).unwrap();
        assert_eq!(
            detect_quasar_lang_version(dir.path()),
            Some("0.1.0".to_string())
        );
    }

    #[test]
    fn detect_version_git_dep_falls_back_to_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_toml = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
quasar-lang = { git = "https://github.com/blueshift-gg/quasar" }
"#;
        let cargo_lock = r#"
[[package]]
name = "quasar-lang"
version = "0.0.0"
source = "git+https://github.com/blueshift-gg/quasar"
"#;
        std::fs::write(dir.path().join("Cargo.toml"), cargo_toml).unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), cargo_lock).unwrap();
        assert_eq!(
            detect_quasar_lang_version(dir.path()),
            Some("0.0.0".to_string())
        );
    }

    #[test]
    fn detect_version_no_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_quasar_lang_version(dir.path()), None);
    }

    #[test]
    fn requirements_known_version() {
        assert!(requirements_for("0.0.0").is_some());
    }

    #[test]
    fn requirements_unknown_version() {
        assert!(requirements_for("99.99.99").is_none());
    }

    #[test]
    fn version_comparison() {
        assert!(version_less_than("1.86.0", "1.87.0"));
        assert!(version_less_than("1.87.0", "2.0.0"));
        assert!(!version_less_than("1.87.0", "1.87.0"));
        assert!(!version_less_than("1.88.0", "1.87.0"));
        assert!(version_less_than("0.9.0", "1.0.0"));
    }

    #[test]
    fn major_minor_extraction() {
        assert_eq!(major_minor("2.1.6"), "2.1");
        assert_eq!(major_minor("1.87.0"), "1.87");
        assert_eq!(major_minor("2.1"), "2.1");
    }
}
