use {
    crate::{
        config::resolve_client_path,
        error::{CliError, CliResult},
        style,
    },
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

pub fn run(all: bool) -> CliResult {
    let (client_dirs, config_warning) = client_dirs_to_clean(resolve_client_path());
    let mut dirs = vec![
        "target/deploy".to_string(),
        "target/profile".to_string(),
        "target/idl".to_string(),
    ];
    for dir in client_dirs {
        dirs.push(dir.to_string_lossy().into_owned());
    }

    if let Some(warning) = config_warning {
        eprintln!("  {}", style::dim(&warning));
    }

    let removed: Vec<&str> = dirs
        .iter()
        .map(String::as_str)
        .filter(|dir| Path::new(dir).exists())
        .collect();

    if removed.is_empty() && !all {
        println!("  {}", style::dim("nothing to clean"));
        return Ok(());
    }

    for dir in &removed {
        if *dir == "target/deploy" {
            // Preserve keypair files — losing a keypair means losing your program address
            clean_deploy_dir()?;
        } else {
            fs::remove_dir_all(Path::new(dir))?;
        }
    }

    if all {
        let output = Command::new("cargo")
            .arg("clean")
            .output()
            .map_err(anyhow::Error::from)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CliError::process_failure(
                format!("cargo clean failed: {}", stderr.trim()),
                output.status.code().unwrap_or(1),
            ));
        }
    }

    println!("  {}", style::success("clean"));
    Ok(())
}

fn client_dirs_to_clean(clients_dir: Result<PathBuf, CliError>) -> (Vec<PathBuf>, Option<String>) {
    let legacy_clients_dir = PathBuf::from("target").join("client");

    match clients_dir {
        Ok(path) if path == legacy_clients_dir => (vec![legacy_clients_dir], None),
        Ok(path) => (vec![path, legacy_clients_dir], None),
        Err(err) => (
            vec![legacy_clients_dir],
            Some(format!(
                "note: could not read clients.path from Quasar.toml ({err}); falling back to \
                 target/client"
            )),
        ),
    }
}

/// Remove everything in target/deploy/ except keypair files.
fn clean_deploy_dir() -> Result<(), std::io::Error> {
    let deploy = Path::new("target/deploy");
    for entry in fs::read_dir(deploy)?.flatten() {
        let path = entry.path();
        let is_keypair = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with("-keypair.json"));

        if !is_keypair {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_only_legacy_dir_when_config_matches_default() {
        let (dirs, warning) = client_dirs_to_clean(Ok(PathBuf::from("target").join("client")));

        assert_eq!(dirs, vec![PathBuf::from("target").join("client")]);
        assert!(warning.is_none());
    }

    #[test]
    fn cleans_custom_and_legacy_dirs_when_config_moves_clients() {
        let (dirs, warning) = client_dirs_to_clean(Ok(PathBuf::from("generated").join("clients")));

        assert_eq!(
            dirs,
            vec![
                PathBuf::from("generated").join("clients"),
                PathBuf::from("target").join("client")
            ]
        );
        assert!(warning.is_none());
    }

    #[test]
    fn falls_back_to_legacy_dir_when_config_is_invalid() {
        let (dirs, warning) = client_dirs_to_clean(Err(CliError::message("invalid Quasar.toml")));

        assert_eq!(dirs, vec![PathBuf::from("target").join("client")]);
        assert!(
            warning.is_some_and(|msg| msg.contains("falling back to target/client")),
            "warning should explain legacy fallback"
        );
    }
}
