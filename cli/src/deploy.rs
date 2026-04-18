use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        style, utils,
    },
    std::{
        path::PathBuf,
        process::{Command, Stdio},
    },
};

pub fn run(
    program_keypair: Option<PathBuf>,
    upgrade_authority: Option<PathBuf>,
    keypair: Option<PathBuf>,
    url: Option<String>,
    skip_build: bool,
) -> CliResult {
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // Build unless skipped
    if !skip_build {
        crate::build::run(false, false, false, None, false)?;
    }

    // Find the .so binary
    let Some(so_path) = utils::find_so(&config, false) else {
        return Err(CliError::message(format!(
            "no compiled binary found for \"{name}\"\n\n  Run quasar build first."
        )));
    };

    // Find the program keypair (check local and workspace target dirs)
    let keypair_path = program_keypair.unwrap_or_else(|| {
        let module = config.module_name();
        utils::find_in_deploy(&format!("{name}-keypair.json"))
            .or_else(|| utils::find_in_deploy(&format!("{module}-keypair.json")))
            .unwrap_or_else(|| {
                PathBuf::from("target")
                    .join("deploy")
                    .join(format!("{name}-keypair.json"))
            })
    });

    if !keypair_path.exists() {
        return Err(CliError::message(format!(
            "program keypair not found: {}\n\n  Run quasar keys new to generate one, or pass \
             --program-keypair explicitly.",
            keypair_path.display()
        )));
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

    if let Some(payer) = &keypair {
        cmd.args(["--keypair", payer.to_str().unwrap_or_default()]);
    }

    if let Some(cluster) = &url {
        cmd.args(["--url", cluster]);
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
            let mut message = String::new();
            if !stderr.is_empty() {
                message.push_str(stderr.trim_end());
            }
            if !stdout.is_empty() {
                if !message.is_empty() {
                    message.push('\n');
                }
                message.push_str(stdout.trim_end());
            }
            if !message.is_empty() {
                message.push_str("\n\n");
            }
            message.push_str("deploy failed");
            Err(CliError::process_failure(
                message,
                o.status.code().unwrap_or(1),
            ))
        }
        Err(e) => Err(CliError::message(format!(
            "failed to run solana program deploy: {e}\n\n  Make sure the solana CLI is installed \
             and configured."
        ))),
    }
}
