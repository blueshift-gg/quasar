use std::process::Command;

/// Ensure the installed `cargo-build-sbf` supports the required
/// platform-tools release.
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
    let bundled = version_text
        .lines()
        .find_map(|line| line.strip_prefix("platform-tools "))
        .unwrap_or("unknown");
    if parse_tools_version(bundled) < parse_tools_version(required) {
        return Err(format!(
            "quasar requires platform-tools {required}, but the installed cargo-build-sbf only \
             supports {bundled}.\nUpdate Agave CLI: agave-install update",
        ));
    }
    Ok(())
}

fn parse_tools_version(version: &str) -> u32 {
    let version = version.strip_prefix('v').unwrap_or(version);
    let (major, minor) = version.split_once('.').unwrap_or(("0", "0"));
    major.parse::<u32>().unwrap_or(0) * 100 + minor.parse::<u32>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::parse_tools_version;

    #[test]
    fn platform_tools_versions_are_ordered_numerically() {
        assert!(parse_tools_version("v1.52") > parse_tools_version("v1.9"));
        assert_eq!(parse_tools_version("unknown"), 0);
    }
}
