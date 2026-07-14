use {
    crate::config::QuasarConfig,
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// Locate the Cargo workspace `target/` directory.
///
/// Falls back to `./target` when we're not inside a Cargo workspace.
pub fn workspace_target_dir() -> PathBuf {
    if let Ok(o) = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .stderr(Stdio::null())
        .output()
    {
        if o.status.success() {
            if let Ok(manifest) = String::from_utf8(o.stdout) {
                if let Some(root) = Path::new(manifest.trim()).parent() {
                    return root.join("target");
                }
            }
        }
    }
    PathBuf::from("target")
}

/// Find the program crate root directory.
///
/// Returns `"."` when `src/lib.rs` exists in the current directory. Otherwise
/// searches common workspace layouts (`programs/<name>`, `<name>`) to locate
/// the crate.
pub fn find_program_crate(config: &QuasarConfig) -> PathBuf {
    if Path::new("src/lib.rs").exists() {
        return PathBuf::from(".");
    }

    let name = &config.project.name;
    let module = config.module_name();

    for candidate in [
        format!("programs/{name}"),
        format!("programs/{module}"),
        name.to_string(),
        module,
    ] {
        if Path::new(&candidate).join("src/lib.rs").exists() {
            return PathBuf::from(candidate);
        }
    }

    // Fallback produces a clear parse error downstream.
    PathBuf::from(".")
}

/// Find the compiled .so in target/deploy/ (and optionally target/profile/).
///
/// Searches both the local `target/` and the workspace root's `target/` so
/// that quasar works when invoked from a workspace member subdirectory.
pub fn find_so(config: &QuasarConfig, include_profile: bool) -> Option<PathBuf> {
    let module = config.module_name();
    let name = &config.project.name;

    let so_names = [
        format!("{name}.so"),
        format!("{module}.so"),
        format!("lib{module}.so"),
    ];

    let ws_target = workspace_target_dir();

    for base in &[PathBuf::from("target"), ws_target.clone()] {
        let deploy = base.join("deploy");
        for so_name in &so_names {
            let path = deploy.join(so_name);
            if path.exists() {
                return Some(path);
            }
        }
    }

    if include_profile {
        for base in &[PathBuf::from("target"), ws_target] {
            let path = base.join("profile").join(format!("{module}.so"));
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Find the unstripped artifact emitted by `cargo build-sbf --debug`.
pub fn find_unstripped_sbf(config: &QuasarConfig) -> Option<PathBuf> {
    let targets = [PathBuf::from("target"), workspace_target_dir()];
    find_unstripped_sbf_in_targets(&targets, &config.module_name())
}

fn find_unstripped_sbf_in_targets(targets: &[PathBuf], module: &str) -> Option<PathBuf> {
    targets
        .iter()
        .map(|target| {
            target
                .join("deploy")
                .join("debug")
                .join(format!("{module}.so.debug"))
        })
        .find(|path| path.is_file())
}

/// Find a file by name inside `target/deploy/`, checking both local and
/// workspace target directories.
pub fn find_in_deploy(filename: &str) -> Option<PathBuf> {
    let ws_target = workspace_target_dir();
    for base in &[PathBuf::from("target"), ws_target] {
        let path = base.join("deploy").join(filename);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use {
        super::find_unstripped_sbf_in_targets,
        std::{fs, path::Path},
        tempfile::tempdir,
    };

    fn write_unstripped(target: &Path, module: &str) -> std::path::PathBuf {
        let artifact = target
            .join("deploy")
            .join("debug")
            .join(format!("{module}.so.debug"));
        fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("create output");
        fs::write(&artifact, b"unstripped").expect("write artifact");
        artifact
    }

    #[test]
    fn finds_unstripped_artifact_in_local_target() {
        let temp = tempdir().expect("tempdir");
        let local_target = temp.path().join("local/target");
        let workspace_target = temp.path().join("workspace/target");
        let artifact = write_unstripped(&local_target, "demo_program");

        assert_eq!(
            find_unstripped_sbf_in_targets(&[local_target, workspace_target], "demo_program"),
            Some(artifact)
        );
    }

    #[test]
    fn finds_unstripped_artifact_in_workspace_target() {
        let temp = tempdir().expect("tempdir");
        let local_target = temp.path().join("member/target");
        let workspace_target = temp.path().join("workspace/target");
        let artifact = write_unstripped(&workspace_target, "demo_program");

        assert_eq!(
            find_unstripped_sbf_in_targets(&[local_target, workspace_target], "demo_program"),
            Some(artifact)
        );
    }
}
