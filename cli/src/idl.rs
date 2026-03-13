use {
    crate::{error::CliResult, IdlCommand},
    quasar_idl::{codegen, parser},
    std::path::{Path, PathBuf},
};

/// Generate IDL, TypeScript client, and Rust client crate for a program.
///
/// Outputs:
/// - `target/idl/<name>.idl.json`
/// - `target/client/typescript/<name>/web3.ts`
/// - `target/client/rust/<name>-client/` (standalone Rust crate)
pub fn generate(crate_path: &Path) -> CliResult {
    // Parse the program
    let parsed = parser::parse_program(crate_path);

    // Generate client code before build_idl consumes parsed
    let client_code = codegen::rust::generate_client(&parsed);
    let client_cargo_toml = codegen::rust::generate_cargo_toml(&parsed.crate_name, &parsed.version);

    // Build the IDL
    let idl = parser::build_idl(parsed);

    // Generate TypeScript clients from IDL (web3.js + kit entrypoints)
    let ts_code = codegen::typescript::generate_ts_client(&idl);
    let ts_kit_code = codegen::typescript::generate_ts_client_kit(&idl);

    // Write IDL JSON to target/idl/
    let idl_dir = PathBuf::from("target").join("idl");
    std::fs::create_dir_all(&idl_dir).expect("Failed to create target/idl directory");

    let idl_path = idl_dir.join(format!("{}.idl.json", idl.metadata.name));
    let json = serde_json::to_string_pretty(&idl).expect("Failed to serialize IDL");
    std::fs::write(&idl_path, &json).expect("Failed to write IDL file");
    println!("{}", idl_path.display());

    // Write TypeScript clients to target/client/typescript/<name>/
    let ts_dir = PathBuf::from("target")
        .join("client")
        .join("typescript")
        .join(&idl.metadata.name);
    std::fs::create_dir_all(&ts_dir)
        .expect("Failed to create target/client/typescript/<name> directory");
    let ts_path = ts_dir.join("web3.ts");
    std::fs::write(&ts_path, &ts_code).expect("Failed to write TS client");
    println!("{}", ts_path.display());
    let ts_kit_path = ts_dir.join("kit.ts");
    std::fs::write(&ts_kit_path, &ts_kit_code).expect("Failed to write TS kit client");
    println!("{}", ts_kit_path.display());

    // Write package.json for the TS client
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
    std::fs::write(ts_dir.join("package.json"), &ts_package_json)
        .expect("Failed to write TS client package.json");

    // Write Rust client as a standalone crate in target/client/rust/<name>-client/
    let crate_name = &idl.metadata.crate_name;
    let client_dir = PathBuf::from("target")
        .join("client")
        .join("rust")
        .join(format!("{}-client", crate_name));
    let client_src_dir = client_dir.join("src");
    std::fs::create_dir_all(&client_src_dir)
        .expect("Failed to create target/client/rust/<name>-client/src directory");

    std::fs::write(client_dir.join("Cargo.toml"), &client_cargo_toml)
        .expect("Failed to write client Cargo.toml");
    std::fs::write(client_src_dir.join("lib.rs"), &client_code)
        .expect("Failed to write client lib.rs");
    println!("{}", client_dir.display());

    Ok(())
}

pub fn run(command: IdlCommand) -> CliResult {
    let crate_path = &command.crate_path;

    if !crate_path.exists() {
        eprintln!("Error: path does not exist: {}", crate_path.display());
        std::process::exit(1);
    }

    generate(crate_path)
}
