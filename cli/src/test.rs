use {
    crate::{config::QuasarConfig, error::CliResult},
    std::process::Command,
};

pub fn run() -> CliResult {
    let config = QuasarConfig::load()?;

    // Build (generates IDL client crate + SBF binary)
    crate::build::run()?;

    if config.has_typescript_tests() {
        run_typescript_tests()
    } else if config.has_rust_tests() {
        run_rust_tests()
    } else {
        println!("No testing framework configured.");
        Ok(())
    }
}

fn run_rust_tests() -> CliResult {
    println!("Running tests...");
    let status = Command::new("cargo").args(["test", "tests::"]).status();

    match status {
        Ok(s) if s.success() => {
            println!("All tests passed.");
            Ok(())
        }
        Ok(s) => {
            eprintln!("Tests failed with exit code: {}", s.code().unwrap_or(1));
            std::process::exit(s.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Failed to run cargo test: {e}");
            std::process::exit(1);
        }
    }
}

fn run_typescript_tests() -> CliResult {
    println!("Running TypeScript tests...");

    // Install dependencies if needed
    if !std::path::Path::new("node_modules").exists() {
        println!("Installing test dependencies...");
        let status = Command::new("npm").args(["install"]).status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!(
                    "npm install failed with exit code: {}",
                    s.code().unwrap_or(1)
                );
                std::process::exit(s.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!("Failed to run npm install: {e}");
                std::process::exit(1);
            }
        }
    }

    let status = Command::new("npm").args(["test"]).status();

    match status {
        Ok(s) if s.success() => {
            println!("All tests passed.");
            Ok(())
        }
        Ok(s) => {
            eprintln!("Tests failed with exit code: {}", s.code().unwrap_or(1));
            std::process::exit(s.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Failed to run npm test: {e}");
            std::process::exit(1);
        }
    }
}
