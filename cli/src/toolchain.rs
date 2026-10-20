use std::process::Command;

/// Check whether sbpf-linker is reachable on PATH.
pub fn has_sbpf_linker() -> bool {
    Command::new("sbpf-linker").arg("--version").output().ok().is_some_and(|o| o.status.success())
}

/// Check availability. The bundled platform-tools version is a default, not
/// the newest version that `--tools-version` can install.
pub fn check_build_sbf() -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["build-sbf", "--version"])
        .output()
        .map_err(|e| format!("cannot run cargo build-sbf: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo-build-sbf is unavailable; install Agave CLI.\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
