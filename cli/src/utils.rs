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

    // Fallback — will produce a clear parse error downstream.
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

pub use quasar_schema::snake_to_pascal;
