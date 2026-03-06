mod codegen;
mod codegen_ts;

use quasar_idl::parser;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let crate_path = PathBuf::from(
        args.get(1)
            .expect("Usage: quasar-idl <path-to-program-crate>"),
    );

    if !crate_path.exists() {
        eprintln!("Error: path does not exist: {}", crate_path.display());
        std::process::exit(1);
    }

    // Parse the program
    let parsed = parser::parse_program(&crate_path);

    // Generate client code before build_idl consumes parsed
    let client_code = codegen::generate_client(&parsed);

    // Build the IDL
    let idl = parser::build_idl(parsed);

    // Generate TypeScript client from IDL
    let ts_code = codegen_ts::generate_ts_client(&idl);

    // Write IDL JSON to target/idl/
    let output_dir = PathBuf::from("target").join("idl");
    std::fs::create_dir_all(&output_dir).expect("Failed to create target/idl directory");

    let idl_path = output_dir.join(format!("{}.idl.json", idl.metadata.name));
    let json = serde_json::to_string_pretty(&idl).expect("Failed to serialize IDL");
    std::fs::write(&idl_path, &json).expect("Failed to write IDL file");
    println!("{}", idl_path.display());

    // Write TypeScript client to target/idl/
    let ts_path = output_dir.join(format!("{}.ts", idl.metadata.name));
    std::fs::write(&ts_path, &ts_code).expect("Failed to write TS client");
    println!("{}", ts_path.display());

    // Write Rust client to target/idl/ (standalone, not injected into the program crate —
    // the #[program] macro generates its own in-crate client module via WriteBytes)
    let client_path = output_dir.join(format!("{}_client.rs", idl.metadata.name));
    std::fs::write(&client_path, &client_code).expect("Failed to write Rust client");
    println!("{}", client_path.display());
}
