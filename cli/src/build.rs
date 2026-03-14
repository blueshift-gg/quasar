use {
    crate::{config::QuasarConfig, error::CliResult},
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

pub fn run(debug: bool) -> CliResult {
    let config = QuasarConfig::load()?;

    // Generate IDL + client crate first (cargo needs the client crate to resolve
    // dev-deps)
    println!("Generating IDL...");
    crate::idl::generate(Path::new("."), config.has_typescript_tests())?;

    // Build SBF
    println!("Building SBF...");
    let status = if config.is_solana_toolchain() {
        if debug {
            Command::new("cargo")
                .arg("build-sbf")
                .arg("--debug")
                .status()
        } else {
            Command::new("cargo").arg("build-sbf").status()
        }
    } else {
        if debug {
            Command::new("cargo")
                .env("RUSTFLAGS", "-C link-arg=--btf -C debuginfo=2")
                .args(["+nightly", "build-bpf"])
                .status()
        } else {
            Command::new("cargo")
                .args(["+nightly", "build-bpf"])
                .status()
        }
    };

    match status {
        Ok(s) if s.success() => {
            println!("Build complete.");
            let program = config.module_name();
            let src = PathBuf::from("target")
                .join("bpfel-unknown-none")
                .join("release")
                .join(format!("lib{}.so", program));
            let dest = PathBuf::from("target")
                .join("deploy")
                .join(format!("lib{}.so", program));
            let _ = fs::copy(&src, &dest);
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
