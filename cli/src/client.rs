use {
    crate::{
        config::resolve_client_path,
        error::{CliError, CliResult},
        output::{commit, PreparedOutput},
        style, ClientCommand,
    },
    quasar_idl::{
        codegen::{self, model::ProgramModel},
        types::Idl,
    },
    std::path::Path,
};

/// Languages that can be generated from an IDL JSON file.
/// Rust codegen requires the parsed AST and is handled by `quasar idl`.
const ALL_LANGUAGES: &[&str] = &["typescript", "python", "golang", "c"];

pub fn run(command: ClientCommand) -> CliResult {
    let clients_path = resolve_client_path()?;
    let idl_path = &command.idl_path;

    if !idl_path.exists() {
        return Err(CliError::message(format!(
            "IDL file not found: {}",
            idl_path.display()
        )));
    }

    let json =
        std::fs::read_to_string(idl_path).map_err(|e| CliError::io_path("read", idl_path, e))?;
    // Spec-version gate before the full parse so an incompatible schema fails
    // with a clear message.
    quasar_idl::types::check_spec(&json).map_err(CliError::message)?;
    let idl: quasar_idl::types::Idl = serde_json::from_str(&json)
        .map_err(|e| CliError::json_parse(format!("IDL file {}", idl_path.display()), e))?;

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
                "c" | "C" => Ok("c"),
                other => Err(CliError::message(format!(
                    "unknown language: '{other}'. Options: typescript, python, golang, c"
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

pub fn generate_clients(idl: &Idl, languages: &[&str], clients_path: &Path) -> CliResult {
    commit(prepare_clients(idl, languages, clients_path)?)
}

pub(crate) fn prepare_clients(
    idl: &Idl,
    languages: &[&str],
    clients_path: &Path,
) -> Result<Vec<PreparedOutput>, CliError> {
    let requested = languages
        .iter()
        .map(|language| match *language {
            "typescript" => Ok(ClientLanguage::TypeScript),
            "python" => Ok(ClientLanguage::Python),
            "golang" => Ok(ClientLanguage::Go),
            "c" => Ok(ClientLanguage::C),
            other => Err(CliError::message(format!(
                "unknown language: '{other}'. Options: typescript, python, golang, c"
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let model = ProgramModel::try_new(idl).map_err(|error| {
        CliError::message(format!("IDL is not safe for client generation: {error}"))
    })?;
    let mut outputs = Vec::new();

    if requested.contains(&ClientLanguage::TypeScript) {
        let ts_dir = clients_path
            .join("typescript")
            .join(model.identity.typescript_dir.as_str());
        outputs.push(PreparedOutput::file(
            ts_dir.join("web3.ts"),
            codegen::typescript::generate_ts_client(idl)
                .map_err(|e| CliError::message(format!("TypeScript web3.js codegen: {e}")))?,
        ));
        outputs.push(PreparedOutput::file(
            ts_dir.join("kit.ts"),
            codegen::typescript::generate_ts_client_kit(idl)
                .map_err(|e| CliError::message(format!("TypeScript Kit codegen: {e}")))?,
        ));
        outputs.push(PreparedOutput::file(
            ts_dir.join("package.json"),
            codegen::typescript::generate_package_json(idl)
                .map_err(|e| CliError::message(format!("TypeScript package codegen: {e}")))?,
        ));
    }

    if requested.contains(&ClientLanguage::Python) {
        let py_dir = clients_path
            .join("python")
            .join(model.identity.python_package.as_str());
        outputs.push(PreparedOutput::file(
            py_dir.join("client.py"),
            codegen::python::generate_python_client(idl)
                .map_err(|e| CliError::message(format!("Python codegen: {e}")))?,
        ));
        outputs.push(PreparedOutput::file(
            py_dir.join("__init__.py"),
            "from .client import *  # noqa: F401,F403\n",
        ));
    }

    if requested.contains(&ClientLanguage::Go) {
        let go_dir = clients_path
            .join("golang")
            .join(model.identity.go_package.as_str());
        outputs.push(PreparedOutput::file(
            go_dir.join("client.go"),
            codegen::golang::generate_go_client(idl)
                .map_err(|e| CliError::message(format!("Go codegen: {e}")))?,
        ));
        outputs.push(PreparedOutput::file(
            go_dir.join("go.mod"),
            codegen::golang::generate_go_mod_for_program(&model),
        ));
    }

    if requested.contains(&ClientLanguage::C) {
        let c_dir = clients_path
            .join("c")
            .join(model.identity.client_name.as_str());
        outputs.push(PreparedOutput::file(
            c_dir.join("client.h"),
            codegen::c::generate_c_client(idl)
                .map_err(|e| CliError::message(format!("C codegen: {e}")))?,
        ));
    }

    Ok(outputs)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClientLanguage {
    TypeScript,
    Python,
    Go,
    C,
}

#[cfg(test)]
mod tests {
    use {
        super::generate_clients,
        quasar_idl::types::{Idl, IdlArg, IdlInstruction, IdlMetadata, IdlType},
        std::{collections::BTreeMap, fs, path::Path},
        tempfile::tempdir,
    };

    fn minimal_idl(name: &str) -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_owned(),
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            address: "11111111111111111111111111111111".to_owned(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, path: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
            let mut entries = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }

        let mut files = BTreeMap::new();
        walk(root, root, &mut files);
        files
    }

    #[test]
    fn hostile_identity_cannot_escape_clients_root() {
        let root = tempdir().unwrap();
        let mut idl = minimal_idl("vault");
        idl.metadata.crate_name = Some("../escape".to_owned());

        let error = generate_clients(&idl, &["typescript"], root.path()).unwrap_err();

        assert!(error.to_string().contains("one path component"));
        assert!(fs::read_dir(root.path()).unwrap().next().is_none());
        assert!(!root.path().parent().unwrap().join("escape").exists());
    }

    #[test]
    fn invalid_later_language_preserves_existing_outputs() {
        let root = tempdir().unwrap();
        let web3 = root.path().join("typescript/vault/web3.ts");
        fs::create_dir_all(web3.parent().unwrap()).unwrap();
        fs::write(&web3, "existing").unwrap();

        let error = generate_clients(&minimal_idl("vault"), &["typescript", "bogus"], root.path())
            .unwrap_err();

        assert!(error.to_string().contains("unknown language"));
        assert_eq!(fs::read_to_string(web3).unwrap(), "existing");
    }

    #[test]
    fn unsupported_external_idl_returns_error_without_panicking() {
        let root = tempdir().unwrap();
        let mut idl = minimal_idl("vault");
        idl.instructions.push(IdlInstruction {
            name: "set_value".to_owned(),
            discriminator: vec![1],
            docs: vec![],
            accounts: vec![],
            args: vec![IdlArg {
                name: "value".to_owned(),
                ty: IdlType::Generic {
                    generic: "T".to_owned(),
                },
                codec: None,
                docs: vec![],
            }],
            layout: None,
            remaining_accounts: None,
        });

        let result = std::panic::catch_unwind(|| generate_clients(&idl, &["python"], root.path()));
        let error = result.expect("external IDL must not panic").unwrap_err();

        assert!(error
            .to_string()
            .contains("does not support generic type `T`"));
        assert!(fs::read_dir(root.path()).unwrap().next().is_none());
    }

    #[test]
    fn successful_generation_is_byte_for_byte_deterministic() {
        let root = tempdir().unwrap();
        let idl = minimal_idl("vault");
        let languages = ["typescript", "python", "golang", "c"];

        generate_clients(&idl, &languages, root.path()).unwrap();
        let first = snapshot(root.path());
        generate_clients(&idl, &languages, root.path()).unwrap();
        let second = snapshot(root.path());

        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
    }
}
