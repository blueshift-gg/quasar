use std::path::PathBuf;

use quasar_idl::{codegen, parser};

use crate::error::CliResult;
use crate::IdlCommand;

pub fn run(command: IdlCommand) -> CliResult {
    let crate_path = &command.crate_path;

    if !crate_path.exists() {
        eprintln!("Error: path does not exist: {}", crate_path.display());
        std::process::exit(1);
    }

    // Parse the program
    let parsed = parser::parse_program(crate_path);

    // Generate client code before build_idl consumes parsed
    let client_code = codegen::rust::generate_client(&parsed);
    let client_cargo_toml =
        codegen::rust::generate_cargo_toml(&parsed.program_name, &parsed.version);

    // Build the IDL
    let idl = parser::build_idl(parsed);

    // Generate TypeScript client from IDL
    let ts_code = codegen::typescript::generate_ts_client(&idl);

    // Write IDL JSON to target/idl/
    let idl_dir = PathBuf::from("target").join("idl");
    std::fs::create_dir_all(&idl_dir).expect("Failed to create target/idl directory");

    let idl_path = idl_dir.join(format!("{}.idl.json", idl.metadata.name));
    let json = serde_json::to_string_pretty(&idl).expect("Failed to serialize IDL");
    std::fs::write(&idl_path, &json).expect("Failed to write IDL file");
    println!("{}", idl_path.display());

    // Write TypeScript client to target/idl/
    let ts_path = idl_dir.join(format!("{}.ts", idl.metadata.name));
    std::fs::write(&ts_path, &ts_code).expect("Failed to write TS client");
    println!("{}", ts_path.display());

    // Write Rust client as a standalone crate in target/rust/<name>-client/
    let client_dir = PathBuf::from("target")
        .join("rust")
        .join(format!("{}-client", idl.metadata.name));
    let client_src_dir = client_dir.join("src");
    std::fs::create_dir_all(&client_src_dir)
        .expect("Failed to create target/rust/<name>-client/src directory");

    std::fs::write(client_dir.join("Cargo.toml"), &client_cargo_toml)
        .expect("Failed to write client Cargo.toml");
    std::fs::write(client_src_dir.join("lib.rs"), &client_code)
        .expect("Failed to write client lib.rs");
    println!("{}", client_dir.display());

    Ok(())
}
