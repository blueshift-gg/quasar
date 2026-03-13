use {
    crate::{config::QuasarConfig, error::CliResult},
    std::{path::Path, process::Command},
};

pub fn run() -> CliResult {
    let config = QuasarConfig::load()?;

    // Generate IDL + client crate first (cargo needs the client crate to resolve
    // dev-deps)
    println!("Generating IDL...");
    crate::idl::generate(Path::new("."))?;

    // Build SBF
    println!("Building SBF...");
    let status = if config.is_solana_toolchain() {
        Command::new("cargo").arg("build-sbf").status()
    } else {
        Command::new("cargo")
            .args(["+nightly", "build-bpf"])
            .status()
    };

    match status {
        Ok(s) if s.success() => {
            println!("Build complete.");
            Ok(())
        }
        Ok(s) => {
            eprintln!("Build failed with exit code: {}", s.code().unwrap_or(1));
            std::process::exit(s.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Failed to run build command: {e}");
            std::process::exit(1);
        }
    }
}
