//! End-to-end round-trip against a real parsed program.
//!
//! Parses a workspace test program via `quasar_idl`, builds an [`Idl`],
//! canonical-encodes it, decodes, and asserts equality. This validates that
//! every `IdlType` / `IdlSeed` / `IdlTypeDefKind` shape the parser emits from
//! real source survives a round-trip — not just hand-crafted fixtures.
//!
//! Uses `test-pda` rather than `test-misc` because `test-misc` intentionally
//! embeds discriminator collisions to exercise the collision detector in
//! `build_idl`, so its `build_idl` call fails by design.

use {
    quasar_idl::parser,
    quasar_schema_canonical::{decode, encode},
    std::path::PathBuf,
};

#[test]
fn test_program_round_trips() {
    // `CARGO_MANIFEST_DIR` points at `.../quasar/schema-canonical`; go up one
    // level then into `tests/programs/test-pda`.
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("schema-canonical has a parent directory")
        .join("tests/programs/test-pda");

    assert!(
        crate_root.exists(),
        "test-pda program not found at {}",
        crate_root.display()
    );

    let parsed = parser::parse_program(&crate_root);
    let idl = parser::build_idl(&parsed)
        .unwrap_or_else(|errs| panic!("build_idl failed:\n{}", errs.join("\n")));

    let blob = encode(&idl);
    let decoded = decode(&blob).expect("canonical decode of real IDL should succeed");
    assert_eq!(idl, decoded, "round-trip must preserve the IDL");

    // Encoding is stable: decoding and re-encoding yields identical bytes.
    let reencoded = encode(&decoded);
    assert_eq!(blob, reencoded, "encoding is byte-for-byte deterministic");
}
