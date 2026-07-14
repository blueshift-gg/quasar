use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        output::{commit, PreparedOutput},
        program_keypair::ProgramKeypair,
        style, utils, VerifyCommand,
    },
    quasar_idl::types::Idl,
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
    },
    tempfile::NamedTempFile,
};

const MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeploymentManifest {
    pub version: u32,
    pub program_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    pub elf_sha256: String,
    pub idl_sha256: String,
    pub abi_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade_authority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_data_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_deploy_slot: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgramShow {
    #[serde(alias = "program_id")]
    program_id: String,
    #[serde(default)]
    authority: Option<String>,
    #[serde(default, alias = "program_data_address")]
    program_data_address: Option<String>,
    #[serde(default, alias = "last_deploy_slot")]
    last_deploy_slot: Option<u64>,
}

struct VerifyRequest<'a> {
    program_id: &'a str,
    elf_path: &'a Path,
    idl: &'a Idl,
    cluster: Option<&'a str>,
    expected_authority: Option<&'a str>,
}

pub fn run(command: VerifyCommand) -> CliResult {
    let config = QuasarConfig::load()?;
    let cluster = resolve_cluster("solana", command.url.as_deref())?;
    let keypair_path = command
        .program_keypair
        .or_else(|| default_program_keypair_path(&config));
    let program_id = match (command.program_id, keypair_path) {
        (Some(program_id), _) => {
            validate_address(&program_id)?;
            program_id
        }
        (None, Some(path)) => ProgramKeypair::read(&path)?.program_id(),
        (None, None) => {
            return Err(CliError::message(
                "program keypair not found; pass `--program-id` or `--program-keypair`",
            ));
        }
    };
    let elf_path = command
        .elf_path
        .or_else(|| utils::find_so(&config, false))
        .ok_or_else(|| {
            CliError::message("compiled program not found; run `quasar build` or pass `--elf-path`")
        })?;
    let (_, idl) = crate::idl::load_generated(&config)?;
    let expected_authority = command
        .upgrade_authority
        .as_deref()
        .map(ProgramKeypair::read)
        .transpose()?
        .map(|keypair| keypair.program_id());

    let observed = verify_live(
        VerifyRequest {
            program_id: &program_id,
            elf_path: &elf_path,
            idl: &idl,
            cluster: Some(&cluster),
            expected_authority: expected_authority.as_deref(),
        },
        "solana",
    )?;

    let explicit_manifest = command.manifest.is_some();
    let manifest_path = command
        .manifest
        .unwrap_or_else(|| default_manifest_path(&config, &elf_path));
    if manifest_path.is_file() {
        let expected = read_manifest(&manifest_path)?;
        validate_manifest(&expected, &observed)?;
        println!(
            "  {}",
            style::success(&format!(
                "Deployment manifest verified: {}",
                manifest_path.display()
            ))
        );
    } else if explicit_manifest {
        return Err(CliError::message(format!(
            "deployment manifest not found: {}",
            manifest_path.display()
        )));
    }

    println!(
        "  {}",
        style::success(&format!("On-chain program verified: {program_id}"))
    );
    Ok(())
}

pub(crate) fn verify_after_deploy(
    config: &QuasarConfig,
    program_id: &str,
    elf_path: &Path,
    idl: &Idl,
    cluster: Option<&str>,
    expected_authority_path: Option<&Path>,
) -> CliResult {
    let expected_authority = expected_authority_path
        .map(ProgramKeypair::read)
        .transpose()?
        .map(|keypair| keypair.program_id());
    let observed = verify_live(
        VerifyRequest {
            program_id,
            elf_path,
            idl,
            cluster,
            expected_authority: expected_authority.as_deref(),
        },
        "solana",
    )?;
    let path = default_manifest_path(config, elf_path);
    write_manifest(&path, &observed)?;
    println!(
        "  {}",
        style::success(&format!("Deployment verified; wrote {}", path.display()))
    );
    Ok(())
}

fn verify_live(request: VerifyRequest<'_>, solana: &str) -> Result<DeploymentManifest, CliError> {
    validate_address(request.program_id)?;
    if !request.elf_path.is_file() {
        return Err(CliError::message(format!(
            "local ELF not found: {}",
            request.elf_path.display()
        )));
    }

    let show = show_program(solana, request.program_id, request.cluster)?;
    if show.program_id != request.program_id {
        return Err(CliError::message(format!(
            "program ID mismatch: requested {}, RPC returned {}",
            request.program_id, show.program_id
        )));
    }
    if let Some(expected) = request.expected_authority {
        if show.authority.as_deref() != Some(expected) {
            return Err(CliError::message(format!(
                "upgrade authority mismatch: expected {expected}, on-chain authority is {}",
                show.authority.as_deref().unwrap_or("none (immutable)")
            )));
        }
    }

    let dump = NamedTempFile::new().map_err(|error| {
        CliError::message(format!("failed to create program dump file: {error}"))
    })?;
    dump_program(solana, request.program_id, dump.path(), request.cluster)?;
    let local_hash = sha256_file(request.elf_path)?;
    let deployed_hash = sha256_file(dump.path())?;
    if local_hash != deployed_hash {
        return Err(CliError::message(format!(
            "deployed ELF mismatch for {}\n  local:    {}\n  on-chain: {}",
            request.program_id, local_hash, deployed_hash
        )));
    }

    let hashes = request.idl.hashes.as_ref().ok_or_else(|| {
        CliError::message("generated IDL has no integrity hashes; rebuild it with `quasar build`")
    })?;
    Ok(DeploymentManifest {
        version: MANIFEST_VERSION,
        program_id: request.program_id.to_string(),
        cluster: request.cluster.map(str::to_owned),
        elf_sha256: local_hash,
        idl_sha256: hashes.idl.clone(),
        abi_sha256: hashes.abi.clone(),
        source_revision: source_revision(),
        upgrade_authority: show.authority,
        program_data_address: show.program_data_address,
        last_deploy_slot: show.last_deploy_slot,
    })
}

fn show_program(
    solana: &str,
    program_id: &str,
    cluster: Option<&str>,
) -> Result<ProgramShow, CliError> {
    let mut args = vec![
        OsString::from("program"),
        OsString::from("show"),
        OsString::from(program_id),
        OsString::from("--output"),
        OsString::from("json"),
    ];
    append_cluster(&mut args, cluster);
    let output = run_solana(solana, &args, "inspect deployed program")?;
    serde_json::from_slice(&output.stdout)
        .map_err(|error| CliError::json_parse("solana program show output", error))
}

fn dump_program(
    solana: &str,
    program_id: &str,
    output_path: &Path,
    cluster: Option<&str>,
) -> CliResult {
    let mut args = vec![
        OsString::from("program"),
        OsString::from("dump"),
        OsString::from(program_id),
        output_path.as_os_str().to_owned(),
    ];
    append_cluster(&mut args, cluster);
    run_solana(solana, &args, "dump deployed program")?;
    Ok(())
}

fn append_cluster(args: &mut Vec<OsString>, cluster: Option<&str>) {
    if let Some(cluster) = cluster {
        args.push(OsString::from("--url"));
        args.push(OsString::from(cluster));
    }
}

fn run_solana(solana: &str, args: &[OsString], action: &str) -> Result<Output, CliError> {
    let output = Command::new(solana)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| CliError::message(format!("failed to run solana CLI: {error}")))?;
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    Err(CliError::process_failure(
        format!("failed to {action}: {detail}"),
        output.status.code().unwrap_or(1),
    ))
}

pub(crate) fn resolve_cluster(solana: &str, requested: Option<&str>) -> Result<String, CliError> {
    if let Some(cluster) = requested {
        return Ok(cluster.to_owned());
    }

    let args = [
        OsString::from("config"),
        OsString::from("get"),
        OsString::from("json_rpc_url"),
        OsString::from("--output"),
        OsString::from("json-compact"),
    ];
    let output = run_solana(solana, &args, "read the configured RPC URL")?;
    parse_configured_rpc_url(&output.stdout).ok_or_else(|| {
        CliError::message("solana config did not report an RPC URL; pass `--url` explicitly")
    })
}

fn parse_configured_rpc_url(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        for key in ["jsonRpcUrl", "json_rpc_url"] {
            if let Some(url) = value.get(key).and_then(serde_json::Value::as_str) {
                if !url.trim().is_empty() {
                    return Some(url.trim().to_owned());
                }
            }
        }
    }

    text.lines().find_map(|line| {
        let url = line.strip_prefix("RPC URL:")?.trim();
        (!url.is_empty()).then(|| url.to_owned())
    })
}

fn validate_manifest(expected: &DeploymentManifest, observed: &DeploymentManifest) -> CliResult {
    if expected.version != MANIFEST_VERSION {
        return Err(CliError::message(format!(
            "unsupported deployment manifest version {}; expected {MANIFEST_VERSION}",
            expected.version
        )));
    }
    let mut mismatches = Vec::new();
    compare(
        &mut mismatches,
        "program ID",
        &expected.program_id,
        &observed.program_id,
    );
    compare(
        &mut mismatches,
        "ELF hash",
        &expected.elf_sha256,
        &observed.elf_sha256,
    );
    compare(
        &mut mismatches,
        "IDL hash",
        &expected.idl_sha256,
        &observed.idl_sha256,
    );
    compare(
        &mut mismatches,
        "ABI hash",
        &expected.abi_sha256,
        &observed.abi_sha256,
    );
    compare(
        &mut mismatches,
        "upgrade authority",
        &expected.upgrade_authority,
        &observed.upgrade_authority,
    );
    compare(
        &mut mismatches,
        "program data address",
        &expected.program_data_address,
        &observed.program_data_address,
    );
    compare(
        &mut mismatches,
        "last deploy slot",
        &expected.last_deploy_slot,
        &observed.last_deploy_slot,
    );
    if observed.cluster.is_some() {
        compare(
            &mut mismatches,
            "cluster",
            &expected.cluster,
            &observed.cluster,
        );
    }
    if expected.source_revision.is_some() && observed.source_revision.is_some() {
        compare(
            &mut mismatches,
            "source revision",
            &expected.source_revision,
            &observed.source_revision,
        );
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(CliError::message(format!(
            "deployment manifest mismatch:\n  {}",
            mismatches.join("\n  ")
        )))
    }
}

fn compare<T: PartialEq + core::fmt::Debug>(
    mismatches: &mut Vec<String>,
    label: &str,
    expected: &T,
    observed: &T,
) {
    if expected != observed {
        mismatches.push(format!(
            "{label}: manifest {expected:?}, observed {observed:?}"
        ));
    }
}

fn write_manifest(path: &Path, manifest: &DeploymentManifest) -> CliResult {
    let mut json = serde_json::to_vec_pretty(manifest)
        .map_err(|error| CliError::json_serialize("deployment manifest", error))?;
    json.push(b'\n');
    commit(vec![PreparedOutput::file(path, json)])
}

fn read_manifest(path: &Path) -> Result<DeploymentManifest, CliError> {
    let json = fs::read(path).map_err(|error| CliError::io_path("read", path, error))?;
    serde_json::from_slice(&json).map_err(|error| {
        CliError::json_parse(format!("deployment manifest {}", path.display()), error)
    })
}

fn default_program_keypair_path(config: &QuasarConfig) -> Option<PathBuf> {
    let name = &config.project.name;
    let module = config.module_name();
    utils::find_in_deploy(&format!("{name}-keypair.json"))
        .or_else(|| utils::find_in_deploy(&format!("{module}-keypair.json")))
}

fn default_manifest_path(config: &QuasarConfig, elf_path: &Path) -> PathBuf {
    let parent = elf_path
        .parent()
        .unwrap_or_else(|| Path::new("target/deploy"));
    parent.join(format!("{}-deployment.json", config.module_name()))
}

fn validate_address(address: &str) -> CliResult {
    let bytes = bs58::decode(address)
        .into_vec()
        .map_err(|_| CliError::message(format!("invalid program address: {address}")))?;
    if bytes.len() != 32 {
        return Err(CliError::message(format!(
            "invalid program address length: expected 32 bytes, found {}",
            bytes.len()
        )));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(|error| CliError::io_path("read", path, error))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn source_revision() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let revision = output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|revision| !revision.is_empty())?;
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .stderr(Stdio::null())
        .output()
        .ok()
        .is_some_and(|status| status.status.success() && !status.stdout.is_empty());
    Some(source_revision_label(&revision, dirty))
}

fn source_revision_label(revision: &str, dirty: bool) -> String {
    if dirty {
        format!("{revision}-dirty")
    } else {
        revision.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> DeploymentManifest {
        DeploymentManifest {
            version: MANIFEST_VERSION,
            program_id: "11111111111111111111111111111111".to_string(),
            cluster: Some("devnet".to_string()),
            elf_sha256: "elf".to_string(),
            idl_sha256: "idl".to_string(),
            abi_sha256: "abi".to_string(),
            source_revision: Some("commit".to_string()),
            upgrade_authority: Some("authority".to_string()),
            program_data_address: Some("data".to_string()),
            last_deploy_slot: Some(7),
        }
    }

    #[test]
    fn parses_program_show_contract() {
        let show: ProgramShow = serde_json::from_str(
            r#"{"programId":"11111111111111111111111111111111","authority":null,"programDataAddress":"data","lastDeploySlot":42}"#,
        )
        .unwrap();
        assert_eq!(show.program_id, "11111111111111111111111111111111");
        assert_eq!(show.authority, None);
        assert_eq!(show.last_deploy_slot, Some(42));
    }

    #[test]
    fn parses_configured_cluster_from_plain_and_json_output() {
        assert_eq!(
            parse_configured_rpc_url(b"RPC URL: http://localhost:8899 \n"),
            Some("http://localhost:8899".to_string())
        );
        assert_eq!(
            parse_configured_rpc_url(br#"{"jsonRpcUrl":"https://api.devnet.solana.com"}"#),
            Some("https://api.devnet.solana.com".to_string())
        );
    }

    #[test]
    fn source_revision_marks_dirty_worktrees() {
        assert_eq!(source_revision_label("abc123", false), "abc123");
        assert_eq!(source_revision_label("abc123", true), "abc123-dirty");
    }

    #[test]
    fn manifest_round_trip_and_comparison() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("deployment.json");
        let expected = manifest();
        write_manifest(&path, &expected).unwrap();
        let observed = read_manifest(&path).unwrap();
        assert_eq!(observed, expected);
        validate_manifest(&expected, &observed).unwrap();
    }

    #[test]
    fn manifest_reports_each_important_mismatch() {
        let expected = manifest();
        let mut observed = expected.clone();
        observed.elf_sha256 = "different".to_string();
        observed.abi_sha256 = "different-abi".to_string();
        observed.program_data_address = Some("different-data".to_string());
        observed.last_deploy_slot = Some(8);
        let error = validate_manifest(&expected, &observed).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("ELF hash"), "{message}");
        assert!(message.contains("ABI hash"), "{message}");
        assert!(message.contains("program data address"), "{message}");
        assert!(message.contains("last deploy slot"), "{message}");
    }

    #[test]
    fn validates_base58_address_length() {
        validate_address("11111111111111111111111111111111").unwrap();
        assert!(validate_address("abc").is_err());
    }
}
