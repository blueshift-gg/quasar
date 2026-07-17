use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        program_keypair::ProgramKeypair,
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
    run_with_verification(
        program_keypair,
        upgrade_authority,
        keypair,
        url,
        skip_build,
        false,
    )
}

pub(crate) fn run_with_verification(
    program_keypair: Option<PathBuf>,
    upgrade_authority: Option<PathBuf>,
    keypair: Option<PathBuf>,
    url: Option<String>,
    skip_build: bool,
    skip_verify: bool,
) -> CliResult {
    let config = QuasarConfig::load()?;
    let name = &config.project.name;
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

    // Validate the signer before handing its path to an external process.
    let program_id = ProgramKeypair::read(&keypair_path)?.program_id();
    let cluster = crate::verify::resolve_cluster("solana", url.as_deref())?;

    if !skip_build {
        crate::build::run(false, false, false, None)?;
    }

    let crate_root = utils::find_program_crate(&config);
    let (_, idl) = crate::idl::load_generated(&config)?;
    // `--skip-build` still receives the same compatibility checks as a normal
    // deployment; it must not become a bypass around the wire-identity gate.
    crate::lint::run_for_build(&crate_root, &idl)?;
    crate::lint::require_deploy_lock(&crate_root, &idl)?;

    let Some(so_path) = utils::find_so(&config, false) else {
        return Err(CliError::message(format!(
            "no compiled binary found for \"{name}\"\n\n  Run quasar build first."
        )));
    };

    let sp = style::spinner("Deploying...");

    let mut cmd = Command::new("solana");
    cmd.args(["program", "deploy"])
        .arg(&so_path)
        .arg("--program-id")
        .arg(&keypair_path);

    if let Some(authority) = &upgrade_authority {
        cmd.arg("--upgrade-authority").arg(authority);
    }

    if let Some(payer) = &keypair {
        cmd.arg("--keypair").arg(payer);
    }

    cmd.args(["--url", &cluster]);

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            println!(
                "\n  {}",
                style::success(&format!("Deployed to {}", style::bold(&program_id)))
            );
            println!();
            if !skip_verify {
                crate::verify::verify_after_deploy(
                    &config,
                    &program_id,
                    &so_path,
                    &idl,
                    Some(&cluster),
                    upgrade_authority.as_deref(),
                )?;
            }
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
