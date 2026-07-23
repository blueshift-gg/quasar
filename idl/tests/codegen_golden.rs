//! Owner-local goldens for the stable client generators.
//!
//! A compact representative program protects Rust, Kit, and Web3 output close
//! to the code that owns it. Preview backends use functional compilation
//! checks when their implementation or the shared model changes; they do not
//! carry patch-level snapshots.
//!
//! Regenerate deliberately with `UPDATE_EXPECT=1 cargo test -p quasar-idl
//! --test codegen_golden` and review every hunk like code.

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

/// A single instruction that derives both a PDA and an associated-token
/// account, so both derivation kinds are exercised by the accessor assertions.
fn address_accessor_idl() -> Idl {
    let account =
        |name: &str, signer: bool, writable: bool, resolver: IdlResolver| IdlAccountNode {
            name: name.to_owned(),
            optional: false,
            writable: AccountFlag::Fixed(writable),
            signer: AccountFlag::Fixed(signer),
            resolver,
            docs: Vec::new(),
        };
    Idl {
        spec: "quasar-idl/1.0.0".to_owned(),
        name: "accessor_demo".to_owned(),
        version: "0.1.0".to_owned(),
        address: "11111111111111111111111111111111".to_owned(),
        metadata: IdlMetadata::default(),
        docs: Vec::new(),
        instructions: vec![IdlInstruction {
            name: "swap".to_owned(),
            discriminator: vec![0],
            docs: Vec::new(),
            accounts: vec![
                account("user", true, true, IdlResolver::Input {}),
                account("mint", false, false, IdlResolver::Input {}),
                account(
                    "pool",
                    false,
                    true,
                    IdlResolver::Pda {
                        program: IdlPdaProgram::ProgramId {},
                        seeds: vec![IdlPdaSeed::Const {
                            value: b"pool".to_vec(),
                        }],
                    },
                ),
                account(
                    "userAta",
                    false,
                    true,
                    IdlResolver::AssociatedToken {
                        mint: "mint".to_owned(),
                        owner: "user".to_owned(),
                        token_program: None,
                    },
                ),
            ],
            args: Vec::new(),
            layout: None,
            remaining_accounts: None,
        }],
        accounts: Vec::new(),
        types: Vec::new(),
        events: Vec::new(),
        errors: Vec::new(),
        extensions: None,
        hashes: None,
    }
}

/// Every PDA and ATA the builder derives must be surfaced on the returned
/// instruction as a `{field}Address` accessor, in both the Kit and Web3
/// clients, carrying the exact address the builder resolved.
#[test]
fn builders_expose_derived_pda_and_ata_addresses() {
    let idl = address_accessor_idl();
    let kit = codegen::typescript::generate_ts_client_kit(&idl).expect("ts kit client");
    let web3 = codegen::typescript::generate_ts_client(&idl).expect("ts web3 client");

    // Kit resolves inside the returned object literal: the return type carries
    // both accessors and each property yields the derived address.
    assert!(kit.contains(
        "Promise<Instruction & { readonly poolAddress: Address; readonly userAtaAddress: \
         Address }>"
    ));
    assert!(kit.contains("poolAddress: (accountOverrides.pool ?? accountsMap[\"pool\"]),"));
    assert!(kit.contains("userAtaAddress: (accountOverrides.userAta ?? accountsMap[\"userAta\"]),"));

    // Web3 attaches the accessors to the constructed class instance.
    assert!(web3.contains(
        "Promise<TransactionInstruction & { readonly poolAddress: Address; readonly \
         userAtaAddress: Address }>"
    ));
    assert!(web3.contains("return Object.assign(instruction, {"));
    assert!(web3.contains("poolAddress: (accountOverrides.pool ?? accountsMap[\"pool\"]),"));
    assert!(
        web3.contains("userAtaAddress: (accountOverrides.userAta ?? accountsMap[\"userAta\"]),")
    );
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
    let instruction: Instruction = MakeInstruction {
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
