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
    quasar_idl::{codegen, types::*},
    std::path::PathBuf,
};

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(name)
}

fn field(name: &str, ty: IdlType) -> IdlFieldDef {
    IdlFieldDef {
        name: name.to_owned(),
        ty,
        codec: None,
        docs: Vec::new(),
    }
}

fn primitive(name: &str) -> IdlType {
    IdlType::Primitive(name.to_owned())
}

/// Compact program exercising the generator surfaces: PDA + input accounts,
/// fixed and dynamic args, a struct account, an enum, an event, and errors.
fn representative_idl() -> Idl {
    Idl {
        spec: "quasar-idl/1.0.0".to_owned(),
        name: "golden_demo".to_owned(),
        version: "0.1.0".to_owned(),
        address: "11111111111111111111111111111111".to_owned(),
        metadata: IdlMetadata::default(),
        docs: Vec::new(),
        instructions: vec![IdlInstruction {
            name: "make".to_owned(),
            docs: Vec::new(),
            discriminator: vec![0],
            args: vec![
                IdlArg {
                    name: "amount".to_owned(),
                    docs: Vec::new(),
                    ty: primitive("u64"),
                    codec: None,
                },
                IdlArg {
                    name: "flag".to_owned(),
                    docs: Vec::new(),
                    ty: primitive("bool"),
                    codec: None,
                },
            ],
            accounts: vec![
                IdlAccountNode {
                    name: "authority".to_owned(),
                    optional: false,
                    writable: AccountFlag::Fixed(false),
                    signer: AccountFlag::Fixed(true),
                    resolver: IdlResolver::Input {},
                    docs: Vec::new(),
                },
                IdlAccountNode {
                    name: "vault".to_owned(),
                    optional: false,
                    writable: AccountFlag::Fixed(true),
                    signer: AccountFlag::Fixed(false),
                    resolver: IdlResolver::Pda {
                        program: IdlPdaProgram::ProgramId {},
                        seeds: vec![
                            IdlPdaSeed::Const {
                                value: b"vault".to_vec(),
                            },
                            IdlPdaSeed::Account {
                                path: "authority".to_owned(),
                            },
                        ],
                    },
                    docs: Vec::new(),
                },
            ],
            remaining_accounts: None,
            layout: None,
        }],
        accounts: vec![IdlAccountDef {
            name: "Vault".to_owned(),
            discriminator: vec![42],
            docs: Vec::new(),
            space: None,
        }],
        types: vec![
            IdlTypeDef {
                name: "Vault".to_owned(),
                kind: IdlTypeDefKind::Struct,
                docs: Vec::new(),
                fields: vec![
                    field("authority", primitive("pubkey")),
                    field("amount", primitive("u64")),
                    field("mode", primitive("u8")),
                ],
                variants: Vec::new(),
                repr: None,
                alias: None,
                fallback: None,
                codec: None,
                layout: None,
                space: None,
                semantics: None,
            },
            IdlTypeDef {
                name: "Mode".to_owned(),
                kind: IdlTypeDefKind::Enum,
                docs: Vec::new(),
                fields: Vec::new(),
                variants: vec![
                    IdlEnumVariant {
                        name: "Open".to_owned(),
                        value: 0,
                        fields: Vec::new(),
                        layout: None,
                    },
                    IdlEnumVariant {
                        name: "Closed".to_owned(),
                        value: 1,
                        fields: Vec::new(),
                        layout: None,
                    },
                ],
                repr: Some("u8".to_owned()),
                alias: None,
                fallback: None,
                codec: None,
                layout: None,
                space: None,
                semantics: None,
            },
        ],
        events: vec![IdlEventDef {
            name: "VaultMade".to_owned(),
            discriminator: vec![7],
            docs: Vec::new(),
            ty: None,
        }],
        errors: vec![IdlErrorDef {
            code: 6000,
            name: "Unauthorized".to_owned(),
            msg: Some("caller is not the vault authority".to_owned()),
        }],
        extensions: None,
        hashes: None,
    }
}

fn rust_client_bundle(idl: &Idl) -> String {
    let files = codegen::rust::generate_client(idl).expect("rust client");
    let mut bundle = String::new();
    for (path, content) in &files {
        bundle.push_str(&format!("//// {path} ////\n{content}\n"));
    }
    bundle
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
