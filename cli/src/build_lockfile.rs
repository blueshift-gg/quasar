use {
    crate::{error::CliError, style},
    serde::Deserialize,
    std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// Read rustflags from .cargo/config.toml for the bpfel-unknown-none target.
pub(super) fn read_target_rustflags() -> Vec<String> {
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

pub(super) fn ensure_lockfile(sp: &indicatif::ProgressBar) -> Result<(), CliError> {
    let lock_path = Path::new("Cargo.lock");
    let lock_exists = lock_path.exists();

    let needs_refresh = if lock_exists {
        fs::metadata(lock_path)
            .and_then(|m| m.modified())
            .ok()
            .is_none_or(|lock_t| {
                workspace_manifest_paths()
                    .map(|paths| {
                        paths
                            .into_iter()
                            .filter_map(|path| fs::metadata(path).and_then(|m| m.modified()).ok())
                            .any(|manifest_t| manifest_t > lock_t)
                    })
                    .unwrap_or(true)
            })
    } else {
        true
    };

    if !needs_refresh {
        return Ok(());
    }

    let result = Command::new("cargo")
        .args(["generate-lockfile", "--quiet"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

    let failed = match result {
        Ok(o) if o.status.success() => return Ok(()),
        Ok(o) => Some(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Some(e.to_string()),
    };

    if !lock_exists {
        sp.finish_and_clear();
        let mut message = String::from("failed to generate Cargo.lock");
        if let Some(msg) = &failed {
            let trimmed = msg.trim();
            if !trimmed.is_empty() {
                message.push('\n');
                message.push_str(trimmed);
            }
        }
        message.push_str(
            "\n\nThe Solana toolchain bundles an older cargo that cannot resolve\nsome newer \
             crate versions. Ensure your system cargo is up to date:\n  rustup update",
        );
        return Err(CliError::message(message));
    }

    eprintln!(
        "  {}",
        style::dim("warning: could not refresh Cargo.lock; building with existing lockfile")
    );
    Ok(())
}

pub(super) fn missing_sbpf_linker() -> CliError {
    CliError::message(
        "sbpf-linker not found on PATH.\n\n  Install platform-tools first:\n    git clone https://github.com/anza-xyz/platform-tools\n    cd platform-tools\n    cargo install-with-gallery",
    )
}

fn workspace_manifest_paths() -> Result<Vec<PathBuf>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(if message.is_empty() {
            "cargo metadata failed".to_string()
        } else {
            message.to_string()
        });
    }

    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let workspace_members: HashSet<_> = metadata.workspace_members.into_iter().collect();
    let mut manifests: Vec<PathBuf> = metadata
        .packages
        .into_iter()
        .filter(|pkg| workspace_members.contains(&pkg.id))
        .map(|pkg| pkg.manifest_path)
        .collect();

    manifests.sort();
    manifests.dedup();
    Ok(manifests)
}

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct CargoMetadataPackage {
    id: String,
    manifest_path: PathBuf,
}
