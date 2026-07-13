use {
    serde_json::Value,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn use_workspace_lang(project_dir: &Path) -> Result<(), Box<dyn Error>> {
    let manifest_path = project_dir.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)?;
    let dependency = manifest
        .lines()
        .find(|line| line.starts_with("quasar-lang = "))
        .ok_or("generated manifest is missing quasar-lang")?;
    let local_dependency = format!(
        "quasar-lang = {{ path = {:?} }}",
        workspace_root().join("lang")
    );
    fs::write(
        manifest_path,
        manifest.replacen(dependency, &local_dependency, 1),
    )?;
    Ok(())
}

#[test]
fn generated_starters_pass_strict_lint() -> Result<(), Box<dyn Error>> {
    let temp = tempdir()?;
    let home = temp.path().join("home");
    fs::create_dir(&home)?;

    for template in ["minimal", "full"] {
        let name = format!("strict-{template}");
        let init = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("init")
            .arg(&name)
            .arg("--yes")
            .arg("--no-git")
            .arg("--test-language")
            .arg("none")
            .arg("--template")
            .arg(template)
            .arg("--toolchain")
            .arg("solana")
            .env("HOME", &home)
            .current_dir(temp.path())
            .output()?;
        assert_success(&format!("quasar init --template {template}"), &init);

        let project_dir = temp.path().join(&name);
        use_workspace_lang(&project_dir)?;

        let lint = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("lint")
            .arg("--strict")
            .arg("--no-diff")
            .current_dir(&project_dir)
            .output()?;
        assert_success(
            &format!("quasar lint --strict for {template} starter"),
            &lint,
        );

        let lib = fs::read_to_string(project_dir.join("src/lib.rs"))?;
        if template == "minimal" {
            assert!(!lib.contains("mod state;"));
            continue;
        }

        let initialize = fs::read_to_string(project_dir.join("src/instructions/initialize.rs"))?;
        assert!(lib.contains("pub mod instructions;"));
        assert!(lib.contains("pub mod state;"));
        assert!(initialize.contains("MyAccount::seeds(payer.address())"));
        assert!(initialize.contains("MyAccountInner"));
        assert!(initialize.contains("_reserved: [0; 64]"));

        let idl = Command::new(env!("CARGO_BIN_EXE_quasar"))
            .arg("idl")
            .arg(".")
            .current_dir(&project_dir)
            .output()?;
        assert_success("quasar idl for full starter", &idl);

        let idl_path = fs::read_dir(project_dir.join("target/idl"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
            .ok_or("full starter did not generate an IDL file")?;
        let idl: Value = serde_json::from_slice(&fs::read(idl_path)?)?;
        let instruction = idl["instructions"]
            .as_array()
            .and_then(|instructions| {
                instructions
                    .iter()
                    .find(|instruction| instruction["name"] == "initialize")
            })
            .ok_or("full starter IDL is missing initialize")?;
        let accounts = instruction["accounts"]
            .as_array()
            .ok_or("initialize accounts should be an array")?;
        let resolver = |name: &str| {
            accounts
                .iter()
                .find(|account| account["name"] == name)
                .map(|account| &account["resolver"])
        };
        assert_eq!(
            resolver("myAccount").map(|value| &value["kind"]),
            Some(&Value::String("pda".into()))
        );
        assert_eq!(
            resolver("systemProgram").map(|value| &value["address"]),
            Some(&Value::String("11111111111111111111111111111111".into())),
        );
        assert_eq!(instruction["args"][0]["name"], "value");

        let client_src = project_dir
            .join("target/client/rust")
            .join(format!("{name}-client/src"));
        let client_initialize = fs::read_to_string(client_src.join("instructions/initialize.rs"))?;
        assert!(client_initialize.contains("pub my_account: Address"));
        assert!(!client_initialize.contains("pub system_program"));
        assert!(client_initialize.contains("11111111111111111111111111111111"));

        let client_pdas = fs::read_to_string(client_src.join("pda.rs"))?;
        assert!(client_pdas.contains("find_my_account_address"));
        assert!(client_pdas.contains("b\"my-account\""));
    }

    Ok(())
}
