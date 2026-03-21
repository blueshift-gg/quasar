use {
    crate::{error::CliResult, style},
    std::process::{Command, Stdio},
};

pub fn run() -> CliResult {
    let current = env!("CARGO_PKG_VERSION");
    eprintln!(
        "  {} Updating Quasar CLI (current: v{current})...",
        style::dim(""),
    );

    let sp = style::spinner("Installing latest quasar-cli...");

    let output = Command::new("cargo")
        .args([
            "install",
            "quasar-cli",
            "--git",
            "https://github.com/blueshift-gg/quasar",
            "--force",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            println!(
                "  {}",
                style::success("Quasar CLI updated successfully.")
            );
            println!();
            let _ = Command::new("quasar").arg("--version").status();
            Ok(())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!();
            for line in stderr.lines() {
                eprintln!("  {line}");
            }
            eprintln!();
            eprintln!("  {}", style::fail("update failed"));
            std::process::exit(o.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "  {}",
                style::fail(&format!("failed to run cargo install: {e}"))
            );
            std::process::exit(1);
        }
    }
}
