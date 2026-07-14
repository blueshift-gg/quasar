use std::process::Command;

/// Actionable recovery guidance shared by every upstream build entry point.
pub const MISSING_SBPF_LINKER_MESSAGE: &str =
    "sbpf-linker not found on PATH.\n\n  Install it from crates.io:\n    cargo install sbpf-linker";

/// Check whether sbpf-linker is reachable on PATH.
pub fn has_sbpf_linker() -> bool {
    command_is_reachable(Command::new("sbpf-linker").arg("--version"))
}

fn command_is_reachable(command: &mut Command) -> bool {
    command.output().is_ok()
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

/// Parse "vX.YZ" into a numeric value for comparison (e.g. "v1.52" to 152).
fn parse_tools_version(s: &str) -> u32 {
    let s = s.strip_prefix('v').unwrap_or(s);
    let (major, minor) = s.split_once('.').unwrap_or(("0", "0"));
    major.parse::<u32>().unwrap_or(0) * 100 + minor.parse::<u32>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use {
        super::{command_is_reachable, MISSING_SBPF_LINKER_MESSAGE},
        std::process::Command,
    };

    #[test]
    fn missing_linker_message_uses_the_published_crate() {
        assert_eq!(
            MISSING_SBPF_LINKER_MESSAGE,
            "sbpf-linker not found on PATH.\n\n  Install it from crates.io:\n    cargo install \
             sbpf-linker"
        );
    }

    #[test]
    fn reachable_linker_does_not_need_a_successful_version_status() {
        assert!(command_is_reachable(
            Command::new("sh").args(["-c", "exit 1"])
        ));
    }

    #[test]
    fn missing_linker_is_not_reachable() {
        assert!(!command_is_reachable(&mut Command::new(
            "quasar-definitely-missing-sbpf-linker"
        )));
    }
}
