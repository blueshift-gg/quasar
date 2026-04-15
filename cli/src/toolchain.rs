use std::process::Command;

/// Check whether sbpf-linker is reachable on PATH.
pub fn has_sbpf_linker() -> bool {
    Command::new("sbpf-linker")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

/// Ensure the installed `cargo-build-sbf` supports the given platform-tools
/// version. Older Agave releases panic with an opaque `unwrap()` when asked
/// for a version they don't know about.
pub fn check_build_sbf_supports(required: &str) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["build-sbf", "--version"])
        .output()
        .map_err(|_| {
            "cargo-build-sbf is not installed.\n\
             Install Agave CLI: https://docs.anza.xyz/cli/install"
                .to_string()
        })?;

    let version_text = String::from_utf8_lossy(&output.stdout);

    // Parse "platform-tools vX.YZ" from the version output.
    let bundled = version_text
        .lines()
        .find_map(|line| line.strip_prefix("platform-tools "))
        .unwrap_or("unknown");

    if parse_tools_version(bundled) < parse_tools_version(required) {
        return Err(format!(
            "quasar requires platform-tools {required}, but the installed cargo-build-sbf only \
             supports {bundled}.\nUpdate Agave CLI:  agave-install update",
        ));
    }
    Ok(())
}

/// Parse "vX.YZ" → numeric value for comparison (e.g. "v1.52" → 152).
fn parse_tools_version(s: &str) -> u32 {
    let s = s.strip_prefix('v').unwrap_or(s);
    let (major, minor) = s.split_once('.').unwrap_or(("0", "0"));
    major.parse::<u32>().unwrap_or(0) * 100 + minor.parse::<u32>().unwrap_or(0)
}
