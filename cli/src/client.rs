use {
    crate::{
        config::resolve_client_path,
        error::{CliError, CliResult},
        style, ClientCommand,
    },
    quasar_idl::codegen,
    std::path::{Path, PathBuf},
};

/// Languages that can be generated from an IDL JSON file.
/// Rust codegen requires the parsed AST and is handled by `quasar idl`.
const ALL_LANGUAGES: &[&str] = &["typescript", "python", "golang"];

pub fn run(command: ClientCommand) -> CliResult {
    let clients_path = resolve_client_path()?;
    let idl_path = &command.idl_path;

    if !idl_path.exists() {
        return Err(CliError::message(format!(
            "IDL file not found: {}",
            idl_path.display()
        )));
    }

    let json = std::fs::read_to_string(idl_path)
        .map_err(|e| anyhow::anyhow!("failed to read IDL: {e}"))?;
    let idl: quasar_idl::types::Idl =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("failed to parse IDL: {e}"))?;

    let languages: Vec<&str> = if command.lang.is_empty() {
        ALL_LANGUAGES.to_vec()
    } else {
        command
            .lang
            .iter()
            .map(|s| match s.as_str() {
                "ts" | "typescript" => Ok("typescript"),
                "py" | "python" => Ok("python"),
                "go" | "golang" => Ok("golang"),
                other => Err(CliError::message(format!(
                    "unknown language: '{other}'. Options: typescript, python, golang"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?
    };

    generate_clients(&idl, &languages, &clients_path)?;

    println!(
        "  {}",
        style::success(&format!("Clients generated: {}", languages.join(", ")))
    );
    Ok(())
}

pub fn generate_clients(
    idl: &quasar_idl::types::Idl,
    languages: &[&str],
    clients_path: &Path,
) -> CliResult {
    // TypeScript
    if languages.contains(&"typescript") {
        let ts_code = codegen::typescript::generate_ts_client(idl);
        let ts_kit_code = codegen::typescript::generate_ts_client_kit(idl);

        let ts_dir = PathBuf::from(clients_path)
            .join("typescript")
            .join(&idl.metadata.name);
        std::fs::create_dir_all(&ts_dir)?;
        std::fs::write(ts_dir.join("web3.ts"), &ts_code)?;
        std::fs::write(ts_dir.join("kit.ts"), &ts_kit_code)?;

        let needs_codecs =
            !idl.types.is_empty() || idl.instructions.iter().any(|ix| !ix.args.is_empty());
        let codecs_dep = if needs_codecs {
            "\n    \"@solana/codecs\": \"^6.2.0\","
        } else {
            ""
        };
        let ts_package_json = format!(
            r#"{{
  "name": "{crate_name}-client",
  "version": "{version}",
  "private": true,
  "exports": {{
    "./web3.js": "./web3.ts",
    "./kit": "./kit.ts"
  }},
  "dependencies": {{{codecs_dep}
    "@solana/kit": "^6.0.0",
    "@solana/web3.js": "github:blueshift-gg/web3.js#v2"
  }}
}}
"#,
            crate_name = idl.metadata.crate_name,
            version = idl.metadata.version,
        );
        std::fs::write(ts_dir.join("package.json"), &ts_package_json)?;
    }

    // Python
    if languages.contains(&"python") {
        let py_code = codegen::python::generate_python_client(idl);
        let py_dir = PathBuf::from(clients_path)
            .join("python")
            .join(&idl.metadata.crate_name);
        std::fs::create_dir_all(&py_dir)?;
        std::fs::write(py_dir.join("client.py"), &py_code)?;
        std::fs::write(
            py_dir.join("__init__.py"),
            "from .client import *  # noqa: F401,F403\n",
        )?;
    }

    // Go
    if languages.contains(&"golang") {
        let go_code = codegen::golang::generate_go_client(idl);
        let go_pkg = idl.metadata.crate_name.replace('-', "_");
        let go_dir = PathBuf::from(clients_path).join("golang").join(&go_pkg);
        std::fs::create_dir_all(&go_dir)?;
        std::fs::write(go_dir.join("client.go"), &go_code)?;
        std::fs::write(
            go_dir.join("go.mod"),
            codegen::golang::generate_go_mod(&go_pkg),
        )?;
    }

    Ok(())
}
