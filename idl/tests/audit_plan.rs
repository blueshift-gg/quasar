//! `audit::validation_plan`: parsing and version-gating of the compiler
//! validation-plan IDL extension. Every rejection pins its diagnostic text —
//! these messages are what `quasar audit` shows users.

use quasar_idl::{audit::validation_plan, types::*};

fn minimal_idl(extensions: Option<serde_json::Value>) -> Idl {
    Idl {
        spec: "quasar-idl/1.0.0".to_owned(),
        name: "audit_demo".to_owned(),
        version: "0.1.0".to_owned(),
        address: "11111111111111111111111111111111".to_owned(),
        metadata: IdlMetadata::default(),
        docs: Vec::new(),
        instructions: Vec::new(),
        accounts: Vec::new(),
        types: Vec::new(),
        events: Vec::new(),
        errors: Vec::new(),
        extensions,
        hashes: None,
    }
}

#[test]
fn accepts_current_version_plan() {
    let idl = minimal_idl(Some(serde_json::json!({
        VALIDATION_EXTENSION_KEY: {
            "version": VALIDATION_EXTENSION_VERSION,
            "instructions": {},
        }
    })));
    let plan = validation_plan(&idl).expect("valid plan");
    assert_eq!(plan.version, VALIDATION_EXTENSION_VERSION);
    assert!(plan.instructions.is_empty());
}

#[test]
fn rejects_idl_without_extensions() {
    let error = validation_plan(&minimal_idl(None)).unwrap_err();
    assert!(
        error.contains("no Quasar validation plan"),
        "diagnostic must say the plan is missing: {error}"
    );
}

#[test]
fn rejects_extensions_without_plan_key() {
    let idl = minimal_idl(Some(serde_json::json!({ "other:extension": {} })));
    let error = validation_plan(&idl).unwrap_err();
    assert!(
        error.contains(VALIDATION_EXTENSION_KEY),
        "diagnostic must name the missing key: {error}"
    );
}

#[test]
fn rejects_malformed_plan() {
    let idl = minimal_idl(Some(serde_json::json!({
        VALIDATION_EXTENSION_KEY: { "version": "not a number" }
    })));
    let error = validation_plan(&idl).unwrap_err();
    assert!(
        error.contains("invalid Quasar validation plan"),
        "diagnostic must report the parse failure: {error}"
    );
}

#[test]
fn rejects_unsupported_version() {
    let idl = minimal_idl(Some(serde_json::json!({
        VALIDATION_EXTENSION_KEY: { "version": 99, "instructions": {} }
    })));
    let error = validation_plan(&idl).unwrap_err();
    assert!(
        error.contains("unsupported validation-plan version 99"),
        "diagnostic must name the found version: {error}"
    );
}
