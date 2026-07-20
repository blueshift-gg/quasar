//! Owner-local goldens for the stable client generators.
//!
//! A compact representative program protects Rust, Kit, and Web3 output close
//! to the code that owns it. Preview backends use functional compilation
//! checks when their implementation or the shared model changes; they do not
//! carry patch-level snapshots.
//!
//! Regenerate deliberately with `UPDATE_EXPECT=1 cargo test -p quasar-idl
//! --test codegen_golden` and review every hunk like code (TESTING.md).

use {
    expect_test::expect_file,
    quasar_idl::{
        codegen::{self, model::ProgramModel},
        types::*,
    },
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(name)
}

/// Compact program exercising the generator surfaces: PDA + input accounts,
/// fixed args, a struct account, an event, and errors.
fn representative_idl() -> Idl {
    serde_json::from_str(include_str!(
        "fixtures/programs/client-conformance.idl.json"
    ))
    .expect("client conformance IDL")
}

fn rust_client_bundle(idl: &Idl) -> String {
    let files = codegen::rust::generate_client(idl).expect("rust client");
    let mut bundle = String::new();
    for (path, content) in &files {
        bundle.push_str(&format!("//// {path} ////\n{content}\n"));
    }
    bundle
}

fn write_rust_client(idl: &Idl, root: &Path) {
    let model = ProgramModel::try_new(idl).expect("client model");
    fs::create_dir_all(root.join("src")).unwrap();
    for (path, content) in codegen::rust::generate_client(idl).expect("rust client") {
        let path = root.join("src").join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let manifest = codegen::rust::generate_cargo_toml_for_program(&model).replace(
        &format!("quasar-lang = \"={}\"", env!("CARGO_PKG_VERSION")),
        &format!("quasar-lang = {{ path = {:?} }}", workspace.join("lang")),
    );
    fs::write(
        root.join("Cargo.toml"),
        format!("{manifest}\n[workspace]\n"),
    )
    .unwrap();
}

#[test]
fn rust_client_matches_golden() {
    let idl = representative_idl();
    let bundle = rust_client_bundle(&idl);
    assert_eq!(
        bundle,
        rust_client_bundle(&idl),
        "generator must be deterministic"
    );
    expect_file![golden("golden_demo.rs.golden")].assert_eq(&bundle);
}

#[test]
fn web3_client_matches_golden() {
    let idl = representative_idl();
    let out = codegen::typescript::generate_ts_client(&idl).expect("ts client");
    assert_eq!(
        out,
        codegen::typescript::generate_ts_client(&idl).expect("ts client"),
        "generator must be deterministic"
    );
    expect_file![golden("golden_demo.ts.golden")].assert_eq(&out);
}

#[test]
fn kit_client_matches_golden() {
    let idl = representative_idl();
    let out = codegen::typescript::generate_ts_client_kit(&idl).expect("ts kit client");
    expect_file![golden("golden_demo.kit.ts.golden")].assert_eq(&out);
}

#[test]
fn stable_typescript_manifests_match_goldens() {
    let idl = representative_idl();
    let kit = codegen::typescript::generate_package_json(&idl, codegen::typescript::TsTarget::Kit)
        .expect("Kit package manifest");
    let web3 =
        codegen::typescript::generate_package_json(&idl, codegen::typescript::TsTarget::Web3js)
            .expect("Web3 package manifest");
    expect_file![golden("golden_demo.kit.package.json.golden")].assert_eq(&kit);
    expect_file![golden("golden_demo.web3.package.json.golden")].assert_eq(&web3);
}

#[test]
fn representative_rust_client_compiles_and_executes_wire_contracts() {
    let root = tempfile::tempdir().unwrap();
    let client = root.path().join("client");
    write_rust_client(&representative_idl(), &client);
    fs::create_dir_all(client.join("tests")).unwrap();
    fs::write(
        client.join("tests/contracts.rs"),
        r#"use {
    golden_demo_client::*,
    solana_address::Address,
    solana_instruction::Instruction,
};

#[test]
fn instruction_account_pda_decoder_error_and_event_contracts() {
    let authority = Address::from([9_u8; 32]);
    let expected_vault = find_vault_address(&authority, &ID).0;
    let instruction: Instruction = MakeInstructionInput {
        authority,
        amount: 42,
        flag: true,
    }
    .into();

    assert_eq!(instruction.accounts.len(), 2);
    assert!(instruction.accounts[0].is_signer);
    assert!(!instruction.accounts[0].is_writable);
    assert_eq!(instruction.accounts[1].pubkey, expected_vault);
    assert!(instruction.accounts[1].is_writable);
    match decode_instruction(&instruction.data) {
        Some(ProgramInstruction::Make { amount, flag }) => {
            assert_eq!(amount, 42);
            assert!(flag);
        }
        _ => panic!("generated instruction did not round trip"),
    }
    assert!(decode_instruction(&[0]).is_none());

    let mut account_data = Vec::new();
    wincode::serialize_into(
        &mut account_data,
        &Vault {
            authority,
            amount: 42,
            mode: 1,
        },
    )
    .unwrap();
    match decode_account(&account_data) {
        Some(ProgramAccount::Vault(vault)) => {
            assert_eq!(vault.authority, authority);
            assert_eq!(vault.amount, 42);
            assert_eq!(vault.mode, 1);
        }
        _ => panic!("generated account did not round trip"),
    }
    assert!(decode_account(&[42]).is_none());

    assert!(matches!(decode_event(&[7]), Some(ProgramEvent::VaultMade)));
    assert!(decode_event(&[7, 0]).is_none());
    assert_eq!(
        GoldenDemoError::from_code(6000).unwrap().message(),
        "caller is not the vault authority"
    );
}
"#,
    )
    .unwrap();

    let output = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated Rust client contract failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
