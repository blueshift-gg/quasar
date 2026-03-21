use {
    crate::{error::CliResult, style},
    std::{fs, path::Path, process::Command},
};

pub fn run(all: bool) -> CliResult {
    let dirs = [
        "target/deploy",
        "target/profile",
        "target/idl",
        "target/client",
    ];

    let removed: Vec<&str> = dirs
        .iter()
        .filter(|d| Path::new(d).exists())
        .copied()
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
            eprintln!(
                "  {}",
                style::fail(&format!("cargo clean failed: {}", stderr.trim()))
            );
            std::process::exit(1);
        }
    }

    println!("  {}", style::success("clean"));
    Ok(())
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
