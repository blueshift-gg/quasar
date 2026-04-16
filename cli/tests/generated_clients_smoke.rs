use {
    quasar_cli::idl,
    quasar_idl::{codegen::model::ProgramModel, parser},
    serde_json::{json, Value},
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    tempfile::tempdir,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn fixture_program() -> PathBuf {
    workspace_root().join("examples/multisig")
}

fn run_command(cmd: &mut Command) -> Result<(), Box<dyn Error>> {
    let output = cmd.output()?;
    if output.status.success() {
        return Ok(());
    }

    let mut message = String::new();
    message.push_str(&format!("command failed: {:?}\n", cmd));
    if !output.stdout.is_empty() {
        message.push_str("stdout:\n");
        message.push_str(&String::from_utf8_lossy(&output.stdout));
        message.push('\n');
    }
    if !output.stderr.is_empty() {
        message.push_str("stderr:\n");
        message.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Err(message.into())
}

fn compile_rust_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .current_dir(client_dir),
    )
}

fn compile_python_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("python3")
            .arg("-m")
            .arg("py_compile")
            .arg("__init__.py")
            .arg("client.py")
            .current_dir(client_dir),
    )
}

fn compile_go_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    run_command(
        Command::new("go")
            .arg("mod")
            .arg("tidy")
            .current_dir(client_dir),
    )?;
    run_command(
        Command::new("go")
            .arg("build")
            .arg("./...")
            .current_dir(client_dir),
    )
}

fn compile_typescript_client(client_dir: &Path) -> Result<(), Box<dyn Error>> {
    // The smoke test validates generated client type-checking, not npm's ability
    // to resolve a GitHub-hosted dependency transport on GitHub runners.
    let package_json_path = client_dir.join("package.json");
    let mut package_json: Value = serde_json::from_str(&fs::read_to_string(&package_json_path)?)?;
    package_json["dependencies"]["@solana/web3.js"] = json!("^1.98.4");
    fs::write(
        &package_json_path,
        serde_json::to_string_pretty(&package_json)? + "\n",
    )?;

    fs::write(
        client_dir.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["web3.ts", "kit.ts"]
}
"#,
    )?;

    run_command(
        Command::new("npm")
            .arg("install")
            .arg("--package-lock=false")
            .arg("--ignore-scripts")
            .arg("--no-audit")
            .arg("--no-fund")
            .arg("typescript")
            .arg("@types/node")
            .current_dir(client_dir),
    )?;

    run_command(
        Command::new("npx")
            .arg("tsc")
            .arg("-p")
            .arg("tsconfig.json")
            .current_dir(client_dir),
    )
}

#[test]
fn generated_clients_compile_from_fresh_project() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_program();
    let parsed = parser::parse_program(&fixture);
    let idl_doc = parser::build_idl(&parsed).map_err(|errors| errors.join("\n"))?;
    let model = ProgramModel::new(&idl_doc);

    let temp = tempdir()?;
    let clients_path = temp.path().join("clients");
    idl::generate(&fixture, &["typescript", "python", "golang"], &clients_path)?;

    compile_rust_client(
        &clients_path
            .join("rust")
            .join(&model.identity.rust_client_crate),
    )?;
    compile_typescript_client(
        &clients_path
            .join("typescript")
            .join(&model.identity.typescript_dir),
    )?;
    compile_python_client(
        &clients_path
            .join("python")
            .join(&model.identity.python_package),
    )?;
    compile_go_client(&clients_path.join("golang").join(&model.identity.go_package))?;

    Ok(())
}
