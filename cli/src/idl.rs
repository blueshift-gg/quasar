use {
    crate::{
        config::resolve_client_path,
        error::{CliError, CliResult},
        IdlCommand,
    },
    quasar_idl::{
        codegen::{self, model::ProgramModel},
        parser::{self, ParsedProgram},
        types::Idl,
    },
    std::path::{Path, PathBuf},
};

/// Parse program source, write IDL JSON and Rust client.
/// Returns the IDL for optional downstream client generation.
fn generate_idl(
    crate_path: &Path,
    clients_path: &Path,
) -> Result<(Idl, ParsedProgram), anyhow::Error> {
    let parsed = parser::parse_program(crate_path);
    let idl =
        parser::build_idl(&parsed).map_err(|errors| anyhow::anyhow!("{}", errors.join("\n")))?;

    // All codegens now work from the IDL — single source of truth.
    let model = ProgramModel::new(&idl);
    let client_code = codegen::rust::generate_client(&idl);
    let client_cargo_toml = codegen::rust::generate_cargo_toml_for_program(&model);

    // Write IDL JSON
    let idl_dir = PathBuf::from("target").join("idl");
    std::fs::create_dir_all(&idl_dir)?;
    let idl_path = idl_dir.join(format!("{}.json", idl.metadata.name));
    let json = serde_json::to_string_pretty(&idl)
        .map_err(|e| anyhow::anyhow!("failed to serialize IDL: {e}"))?;
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

    // No re-parse needed — build_idl borrows, parsed survives for lint.
    Ok((idl, parsed))
}

/// Called by `quasar idl <path>` — generates IDL JSON + Rust client only.
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

/// Called by `quasar build` — generates IDL + Rust client + configured language
/// clients. Returns the ParsedProgram for downstream lint use.
pub fn generate(
    crate_path: &Path,
    languages: &[&str],
    clients_path: &Path,
) -> Result<ParsedProgram, CliError> {
    let (idl, parsed) = generate_idl(crate_path, clients_path)?;
    crate::client::generate_clients(&idl, languages, clients_path)?;
    Ok(parsed)
}
