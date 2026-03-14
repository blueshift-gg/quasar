use {
    crate::{config::QuasarConfig, error::CliResult, style},
    std::{
        path::PathBuf,
        process::{Command, Stdio},
    },
};

pub fn run(program_keypair: Option<PathBuf>, upgrade_authority: Option<PathBuf>) -> CliResult {
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // Find the .so binary
    let so_path = find_so(&config).unwrap_or_else(|| {
        eprintln!(
            "\n  {}",
            style::fail(&format!("no compiled binary found for \"{name}\""))
        );
        eprintln!();
        eprintln!("  Run {} first.", style::bold("quasar build"));
        eprintln!();
        std::process::exit(1);
    });

    // Find the program keypair
    let keypair_path = program_keypair.unwrap_or_else(|| {
        let default = PathBuf::from("target")
            .join("deploy")
            .join(format!("{}-keypair.json", name));
        if !default.exists() {
            // Try module name (underscores)
            let module = config.module_name();
            let alt = PathBuf::from("target")
                .join("deploy")
                .join(format!("{module}-keypair.json"));
            if alt.exists() {
                return alt;
            }
        }
        default
    });

    if !keypair_path.exists() {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program keypair not found: {}",
                keypair_path.display()
            ))
        );
        eprintln!();
        eprintln!(
            "  Run {} to generate one, or pass {} explicitly.",
            style::bold("quasar build"),
            style::bold("--program-keypair")
        );
        eprintln!();
        std::process::exit(1);
    }

    let sp = style::spinner("Deploying...");

    let mut cmd = Command::new("solana");
    cmd.args([
        "program",
        "deploy",
        so_path.to_str().unwrap_or_default(),
        "--program-id",
        keypair_path.to_str().unwrap_or_default(),
    ]);

    if let Some(authority) = &upgrade_authority {
        cmd.args([
            "--upgrade-authority",
            authority.to_str().unwrap_or_default(),
        ]);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);

            // Extract program ID from solana CLI output
            let program_id = stdout
                .lines()
                .find(|l| l.contains("Program Id:"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim())
                .unwrap_or("(unknown)");

            println!(
                "\n  {}",
                style::success(&format!("Deployed to {}", style::bold(program_id)))
            );
            println!();
            Ok(())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let stdout = String::from_utf8_lossy(&o.stdout);
            if !stderr.is_empty() {
                eprintln!();
                for line in stderr.lines() {
                    eprintln!("  {line}");
                }
            }
            if !stdout.is_empty() {
                for line in stdout.lines() {
                    eprintln!("  {line}");
                }
            }
            eprintln!();
            eprintln!("  {}", style::fail("deploy failed"));
            std::process::exit(o.status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "\n  {}",
                style::fail(&format!("failed to run solana program deploy: {e}"))
            );
            eprintln!();
            eprintln!(
                "  Make sure the {} CLI is installed and configured.",
                style::bold("solana")
            );
            eprintln!();
            std::process::exit(1);
        }
    }
}

fn find_so(config: &QuasarConfig) -> Option<PathBuf> {
    let module = config.module_name();
    let name = &config.project.name;
    [
        format!("target/deploy/{name}.so"),
        format!("target/deploy/{module}.so"),
        format!("target/deploy/lib{module}.so"),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}
