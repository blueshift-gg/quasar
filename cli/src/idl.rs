use {
    crate::{
        config::resolve_client_path,
        config::QuasarConfig,
        error::{CliError, CliResult},
        output::{commit, PreparedOutput},
        IdlCommand,
    },
    quasar_idl::{
        codegen::{self, model::ProgramModel},
        types::Idl,
    },
    std::{
        path::{Path, PathBuf},
        process::Command,
    },
};

const IDL_JSON_BEGIN: &str = "__QUASAR_IDL_JSON_BEGIN__";
const IDL_JSON_END: &str = "__QUASAR_IDL_JSON_END__";

fn extract_idl_json(stdout: &str) -> Result<&str, CliError> {
    let (_, after_begin) = stdout.split_once(IDL_JSON_BEGIN).ok_or_else(|| {
        CliError::message(format!(
            "IDL build output did not contain the `{IDL_JSON_BEGIN}` marker"
        ))
    })?;
    let (json, _) = after_begin.split_once(IDL_JSON_END).ok_or_else(|| {
        CliError::message(format!(
            "IDL build output did not contain the `{IDL_JSON_END}` marker"
        ))
    })?;

    let json = json.trim();
    if json.is_empty() {
        return Err(CliError::message(
            "IDL build output contained an empty IDL JSON payload",
        ));
    }

    Ok(json)
}

/// Build the IDL by compiling the program crate with `--features idl-build`
/// and running the `__quasar_emit_idl` test to capture the JSON output.
pub fn build(crate_path: &Path) -> Result<Idl, CliError> {
    // Read the crate name from Cargo.toml
    let cargo_toml_path = crate_path.join("Cargo.toml");
    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)
        .map_err(|e| CliError::io_path("read", &cargo_toml_path, e))?;
    let cargo_toml: toml::Value = cargo_toml_content.parse().map_err(|e| {
        CliError::message(format!(
            "failed to parse {}: {e}",
            cargo_toml_path.display()
        ))
    })?;
    let package_name = cargo_toml
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| {
            CliError::message(format!(
                "missing [package].name in {}",
                cargo_toml_path.display()
            ))
        })?;

    // Run the IDL emission test
    let output = Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(&cargo_toml_path)
        .arg("--features")
        .arg("idl-build")
        .arg("--")
        .arg("__quasar_emit_idl")
        .arg("--nocapture")
        .output()
        .map_err(|e| CliError::message(format!("failed to run cargo test: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("does not contain this feature: idl-build") {
            return Err(CliError::message(format!(
                "IDL build failed because package `{package_name}` does not define the \
                 `idl-build` feature.\n\nAdd this to Cargo.toml:\n\n[features]\nidl-build = \
                 [\"quasar-lang/idl-build\"]\n\ncargo stderr:\n{stderr}"
            )));
        }
        return Err(CliError::message(format!(
            "IDL build failed (cargo test --features idl-build):\n{stderr}"
        )));
    }

    // Parse stdout from the host-only IDL emission test.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = extract_idl_json(&stdout)?;
    let idl: Idl = serde_json::from_str(json_str)
        .map_err(|e| CliError::json_parse("IDL JSON emitted by __quasar_emit_idl", e))?;

    Ok(idl)
}

/// Generate IDL JSON and Rust client from the program crate.
fn generate_idl(crate_path: &Path, clients_path: &Path) -> Result<Idl, CliError> {
    let idl = build(crate_path)?;
    commit(prepare_idl_outputs(&idl, clients_path)?)?;
    Ok(idl)
}

fn prepare_idl_outputs(idl: &Idl, clients_path: &Path) -> Result<Vec<PreparedOutput>, CliError> {
    let model = ProgramModel::try_new(idl).map_err(|error| {
        CliError::message(format!("IDL is not safe for client generation: {error}"))
    })?;
    let client_code = codegen::rust::generate_client(idl)
        .map_err(|error| CliError::message(format!("Rust codegen: {error}")))?;
    let client_cargo_toml = codegen::rust::generate_cargo_toml_for_program(&model);

    let idl_dir = PathBuf::from("target").join("idl");
    let idl_path = idl_dir.join(format!("{}.json", model.identity.program_name));
    // Single presentation writer. The full-IDL hash separately canonicalizes
    // object order and whitespace before hashing.
    let json = quasar_idl::types::canonical_json_pretty(idl)
        .map_err(|e| CliError::json_serialize("IDL JSON", e))?;

    let client_dir = clients_path
        .join("rust")
        .join(model.identity.rust_client_crate.as_str());
    let src_files = client_code
        .into_iter()
        .map(|(path, contents)| (PathBuf::from(path), contents.into_bytes()))
        .collect();

    Ok(vec![
        PreparedOutput::file(idl_path, json),
        PreparedOutput::file(client_dir.join("Cargo.toml"), client_cargo_toml),
        PreparedOutput::directory(client_dir.join("src"), src_files),
    ])
}

/// Locate and parse the IDL emitted by the most recent build.
///
/// Deployment and verification consume the persisted artifact rather than
/// rebuilding the host-only IDL a second time. Both project and module names
/// are checked because Cargo package names may contain hyphens while program
/// modules use underscores.
pub(crate) fn load_generated(config: &QuasarConfig) -> Result<(PathBuf, Idl), CliError> {
    let module_name = config.module_name();
    let names = [&config.project.name, &module_name];
    let workspace_target = crate::utils::workspace_target_dir();
    let roots = [PathBuf::from("target"), workspace_target];

    for root in roots {
        for name in names {
            let path = root.join("idl").join(format!("{name}.json"));
            if !path.is_file() {
                continue;
            }
            let json = std::fs::read_to_string(&path)
                .map_err(|error| CliError::io_path("read", &path, error))?;
            quasar_idl::types::check_spec(&json).map_err(CliError::message)?;
            let idl = serde_json::from_str(&json).map_err(|error| {
                CliError::json_parse(format!("IDL file {}", path.display()), error)
            })?;
            return Ok((path, idl));
        }
    }

    Err(CliError::message(
        "generated IDL not found in target/idl\n\n  Run `quasar build` before deployment or verification.",
    ))
}

/// Called by `quasar idl <path>` (generate) or `quasar idl verify <idl>`.
pub fn run(command: IdlCommand) -> CliResult {
    if let Some(crate::IdlAction::Verify { idl_path }) = &command.action {
        return verify(idl_path);
    }

    let clients_path = resolve_client_path()?;
    let crate_path = command.crate_path.ok_or_else(|| {
        CliError::message("missing PATH: pass a program crate directory, or `verify <IDL>`")
    })?;
    if !crate_path.exists() {
        return Err(CliError::message(format!(
            "path does not exist: {}",
            crate_path.display()
        )));
    }

    generate_idl(&crate_path, &clients_path)?;
    println!("  {}", crate::style::success("IDL generated"));
    Ok(())
}

/// Re-read an IDL JSON, recompute its `hashes.idl` / `hashes.abi`, and check
/// them against the stored values so tampering or drift is caught.
fn verify(idl_path: &Path) -> CliResult {
    if !idl_path.exists() {
        return Err(CliError::message(format!(
            "IDL file not found: {}",
            idl_path.display()
        )));
    }
    let json =
        std::fs::read_to_string(idl_path).map_err(|e| CliError::io_path("read", idl_path, e))?;
    quasar_idl::types::check_spec(&json).map_err(CliError::message)?;
    let idl: Idl = serde_json::from_str(&json)
        .map_err(|e| CliError::json_parse(format!("IDL file {}", idl_path.display()), e))?;

    let stored = idl.hashes.clone().ok_or_else(|| {
        CliError::message(format!(
            "IDL `{}` has no `hashes` field to verify",
            idl_path.display()
        ))
    })?;
    let expected_idl = quasar_idl::types::compute_idl_hash(&idl);
    let expected_abi = quasar_idl::types::compute_abi_hash(&idl);

    let mut mismatches = Vec::new();
    if stored.idl != expected_idl {
        mismatches.push(format!(
            "  idl: stored {} != recomputed {expected_idl}",
            stored.idl
        ));
    }
    if stored.abi != expected_abi {
        mismatches.push(format!(
            "  abi: stored {} != recomputed {expected_abi}",
            stored.abi
        ));
    }

    if mismatches.is_empty() {
        println!(
            "  {}",
            crate::style::success(&format!("IDL hashes verified: {}", idl_path.display()))
        );
        Ok(())
    } else {
        Err(CliError::message(format!(
            "IDL hash mismatch in {}:\n{}",
            idl_path.display(),
            mismatches.join("\n")
        )))
    }
}

/// Called by `quasar build`; generates IDL, Rust client, and configured
/// language clients.
pub fn generate(
    crate_path: &Path,
    languages: &[&str],
    clients_path: &Path,
) -> Result<Idl, CliError> {
    let idl = build(crate_path)?;
    let mut outputs = prepare_idl_outputs(&idl, clients_path)?;
    outputs.extend(crate::client::prepare_clients(
        &idl,
        languages,
        clients_path,
    )?);
    commit(outputs)?;
    Ok(idl)
}

#[cfg(test)]
mod tests {
    use super::{extract_idl_json, IDL_JSON_BEGIN, IDL_JSON_END};

    #[test]
    fn extracts_sentinel_delimited_idl_json() {
        let stdout = format!(
            "running 1 test\nlog {{ not idl \
             }}\n{IDL_JSON_BEGIN}\n{{\"name\":\"demo\"}}\n{IDL_JSON_END}\ntest result: ok"
        );

        assert_eq!(extract_idl_json(&stdout).unwrap(), "{\"name\":\"demo\"}");
    }

    #[test]
    fn rejects_output_without_idl_sentinel() {
        let err = extract_idl_json("running 1 test\n{\"name\":\"demo\"}")
            .expect_err("missing sentinel should fail");

        assert!(
            err.to_string().contains(IDL_JSON_BEGIN),
            "missing begin marker error should be explicit: {err}"
        );
    }
}
