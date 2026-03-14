use {
    crate::{error::CliResult, style},
    std::{fs, path::Path},
};

pub fn run() -> CliResult {
    let dirs = ["target/deploy", "target/profile", "target/idl", "target/client"];

    let removed: Vec<&str> = dirs
        .iter()
        .filter(|d| Path::new(d).exists())
        .copied()
        .collect();

    if removed.is_empty() {
        println!("  {}", style::dim("nothing to clean"));
        return Ok(());
    }

    for dir in &removed {
        fs::remove_dir_all(Path::new(dir))?;
    }

    println!("  {}", style::success("clean"));
    Ok(())
}
