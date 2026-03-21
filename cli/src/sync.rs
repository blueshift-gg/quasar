use {
    crate::{error::CliResult, style, toolchain},
    std::{
        path::Path,
        process::{Command, Stdio},
    },
};

pub fn run() -> CliResult {
    if !Path::new("Cargo.lock").exists() {
        let sp = style::spinner("Generating lockfile...");
        let output = Command::new("cargo")
            .arg("generate-lockfile")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        sp.finish_and_clear();

        match output {
            Ok(o) if o.status.success() => {}
            _ => {
                eprintln!(
                    "  {}",
                    style::fail("Failed to generate Cargo.lock. Is this a valid Cargo project?")
                );
                std::process::exit(1);
            }
        }
    }

    let version = match toolchain::detect_quasar_lang_version(Path::new(".")) {
        Some(v) => v,
        None => {
            eprintln!(
                "  {}",
                style::fail("Could not detect quasar-lang version.")
            );
            eprintln!();
            eprintln!("  Is this a Quasar project?");
            std::process::exit(1);
        }
    };

    println!();
    println!(
        "  {} quasar-lang v{version}",
        style::dim("Detected:"),
    );

    match toolchain::requirements_for(&version) {
        Some(reqs) => {
            println!(
                "  {} Solana CLI v{}, Rust >= v{}",
                style::dim("Required:"),
                reqs.solana_version,
                reqs.rust_version,
            );
        }
        None => {
            println!(
                "  {} unknown (run quasar update)",
                style::dim("Required:"),
            );
        }
    }

    if let Some(ref installed) = toolchain::installed_solana_version() {
        println!(
            "  {} Solana CLI v{installed}",
            style::dim("Installed:"),
        );
    }
    if let Some(ref installed) = toolchain::installed_rust_version() {
        println!(
            "  {}  Rust v{installed}",
            style::dim("Installed:"),
        );
    }
    println!();

    toolchain::ensure_toolchain(Path::new("."));

    println!(
        "  {}",
        style::success("Toolchain is ready.")
    );
    println!();

    Ok(())
}
