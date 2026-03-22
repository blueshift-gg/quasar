use {
    crate::{config::QuasarConfig, error::CliResult, style, utils},
    std::{
        path::PathBuf,
        process::{Command, Stdio},
    },
};

/// Resolve the program keypair path, falling back to target/deploy/<name>-keypair.json.
fn resolve_program_keypair(config: &QuasarConfig, program_keypair: Option<PathBuf>) -> PathBuf {
    program_keypair.unwrap_or_else(|| {
        let name = &config.project.name;
        let default = PathBuf::from("target")
            .join("deploy")
            .join(format!("{}-keypair.json", name));
        if !default.exists() {
            let module = config.module_name();
            let alt = PathBuf::from("target")
                .join("deploy")
                .join(format!("{module}-keypair.json"));
            if alt.exists() {
                return alt;
            }
        }
        default
    })
}

/// Parse and validate a base58 multisig address.
fn parse_multisig_address(addr: &str) -> Result<solana_address::Address, crate::error::CliError> {
    let bytes: [u8; 32] = bs58::decode(addr)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("invalid multisig address: {e}"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("multisig address must be 32 bytes"))?;
    Ok(solana_address::Address::from(bytes))
}

/// Build unless skipped, then locate the compiled .so binary.
fn build_and_find_so(
    config: &QuasarConfig,
    name: &str,
    skip_build: bool,
) -> Result<PathBuf, crate::error::CliError> {
    if !skip_build {
        crate::build::run(false, false, None)?;
    }
    utils::find_so(config, false).ok_or_else(|| {
        eprintln!(
            "\n  {}",
            style::fail(&format!("no compiled binary found for \"{name}\""))
        );
        eprintln!();
        eprintln!("  Run {} first.", style::bold("quasar build"));
        eprintln!();
        std::process::exit(1);
    })
}

/// Run `solana program deploy`.
fn solana_deploy(
    so_path: &std::path::Path,
    program_keypair: &std::path::Path,
    upgrade_authority: Option<&std::path::Path>,
    payer_keypair: Option<&std::path::Path>,
    url: Option<&str>,
) -> CliResult {
    let sp = style::spinner("Deploying...");

    let mut cmd = Command::new("solana");
    cmd.args([
        "program",
        "deploy",
        so_path.to_str().unwrap_or_default(),
        "--program-id",
        program_keypair.to_str().unwrap_or_default(),
    ]);

    if let Some(authority) = upgrade_authority {
        cmd.args([
            "--upgrade-authority",
            authority.to_str().unwrap_or_default(),
        ]);
    }

    if let Some(payer) = payer_keypair {
        cmd.args(["--keypair", payer.to_str().unwrap_or_default()]);
    }

    if let Some(cluster) = url {
        cmd.args(["--url", cluster]);
    }

    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    sp.finish_and_clear();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
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

pub struct DeployOpts {
    pub program_keypair: Option<PathBuf>,
    pub upgrade_authority: Option<PathBuf>,
    pub keypair: Option<PathBuf>,
    pub url: Option<String>,
    pub skip_build: bool,
    pub multisig: Option<String>,
    pub status: bool,
    pub upgrade: bool,
}

pub fn run(opts: DeployOpts) -> CliResult {
    let DeployOpts {
        program_keypair,
        upgrade_authority,
        keypair,
        url,
        skip_build,
        multisig,
        status,
        upgrade,
    } = opts;
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // --upgrade --multisig: Squads proposal flow
    if upgrade {
        if let Some(multisig_addr) = &multisig {
            let multisig_key = parse_multisig_address(multisig_addr)?;
            let payer_path = crate::multisig::solana_keypair_path(keypair.as_deref());
            let rpc_url = crate::multisig::solana_rpc_url(url.as_deref());

            if status {
                return crate::multisig::show_proposal_status(
                    &multisig_key,
                    &payer_path,
                    &rpc_url,
                );
            }

            let so_path = build_and_find_so(&config, name, skip_build)?;
            let prog_keypair_path = resolve_program_keypair(&config, program_keypair);
            let program_id =
                crate::multisig::read_program_id_from_keypair(&prog_keypair_path)?;

            return crate::multisig::propose_upgrade(
                &so_path,
                &program_id,
                &multisig_key,
                &payer_path,
                &rpc_url,
                0,
            );
        }
    }

    // Resolve cluster URL once — handles shorthands like "localnet" that
    // the Solana CLI doesn't understand natively.
    let rpc_url = crate::multisig::solana_rpc_url(url.as_deref());

    // Everything below needs a build and a .so
    let so_path = build_and_find_so(&config, name, skip_build)?;
    let keypair_path = resolve_program_keypair(&config, program_keypair);

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
            style::bold("quasar keys new"),
            style::bold("--program-keypair")
        );
        eprintln!();
        std::process::exit(1);
    }

    // Read program ID from the keypair for on-chain check
    let program_id = crate::multisig::read_program_id_from_keypair(&keypair_path)?;

    // If NOT --upgrade, verify the program doesn't already exist on-chain
    if !upgrade && crate::multisig::program_exists_on_chain(&rpc_url, &program_id)? {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program already deployed at {}",
                bs58::encode(program_id).into_string()
            ))
        );
        eprintln!();
        eprintln!(
            "  Use {} to upgrade an existing program.",
            style::bold("quasar deploy --upgrade")
        );
        eprintln!();
        std::process::exit(1);
    }

    // Deploy (or upgrade) via solana CLI
    solana_deploy(
        &so_path,
        &keypair_path,
        upgrade_authority.as_deref(),
        keypair.as_deref(),
        Some(&rpc_url),
    )?;

    // --multisig without --upgrade: transfer authority to vault after deploy
    if let Some(multisig_addr) = &multisig {
        let multisig_key = parse_multisig_address(multisig_addr)?;
        let (vault, _) = crate::multisig::vault_pda(&multisig_key, 0);
        let payer_path = crate::multisig::solana_keypair_path(keypair.as_deref());

        let sp = style::spinner("Transferring upgrade authority to multisig vault...");
        crate::multisig::set_upgrade_authority(
            &program_id,
            &vault,
            &payer_path,
            &rpc_url,
        )?;
        sp.finish_and_clear();

        println!(
            "  {}",
            style::success(&format!(
                "Upgrade authority transferred to vault {}",
                style::bold(&crate::multisig::short_addr(&vault))
            ))
        );
        println!();
        println!(
            "  Future upgrades: {}",
            style::dim(&format!(
                "quasar deploy --upgrade --multisig {multisig_addr}"
            ))
        );
    }

    println!();
    Ok(())
}
