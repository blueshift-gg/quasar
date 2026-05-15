use {
    crate::{
        config::resolve_client_path,
        error::{CliError, CliResult},
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
fn build_idl_from_crate(crate_path: &Path) -> Result<Idl, CliError> {
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
    let idl = build_idl_from_crate(crate_path)?;

    // Generate client code from the IDL
    let model = ProgramModel::new(&idl);
    let client_code = codegen::rust::generate_client(&idl);
    let client_cargo_toml = codegen::rust::generate_cargo_toml_for_program(&model);

    // Write IDL JSON
    let idl_dir = PathBuf::from("target").join("idl");
    std::fs::create_dir_all(&idl_dir)?;
    let idl_path = idl_dir.join(format!("{}.json", idl.name));
    let json =
        serde_json::to_string_pretty(&idl).map_err(|e| CliError::json_serialize("IDL JSON", e))?;
    std::fs::write(&idl_path, &json)?;

    // Write Rust client
    let client_dir = clients_path
        .join("rust")
        .join(&model.identity.rust_client_crate);
    std::fs::create_dir_all(&client_dir)?;
    std::fs::write(client_dir.join("Cargo.toml"), &client_cargo_toml)?;

    let src_dir = client_dir.join("src");
    if src_dir.exists() {
        std::fs::remove_dir_all(&src_dir)?;
    }
    for (path, content) in &client_code {
        let file_path = src_dir.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, content)?;
    }

    Ok(idl)
}

/// Called by `quasar idl <path>`; generates IDL JSON and the Rust client only.
pub fn run(command: IdlCommand) -> CliResult {
    let clients_path = resolve_client_path()?;
    let crate_path = &command.crate_path;
    if !crate_path.exists() {
        return Err(CliError::message(format!(
            "path does not exist: {}",
            crate_path.display()
        )));
    }

    generate_idl(crate_path, &clients_path)?;
    println!("  {}", crate::style::success("IDL generated"));
    Ok(())
}

/// Called by `quasar build`; generates IDL, Rust client, and configured
/// language clients.
pub fn generate(
    crate_path: &Path,
    languages: &[&str],
    clients_path: &Path,
) -> Result<(), CliError> {
    let idl = generate_idl(crate_path, clients_path)?;
    crate::client::generate_clients(&idl, languages, clients_path)?;
    Ok(())
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
