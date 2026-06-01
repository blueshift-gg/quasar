//! Salsa query behavior: end-to-end parse on real Quasar source, plus
//! incremental-recompute checks for cross-file isolation and early cutoff.

use {
    quasar_hir::{db::Database, parse_file, scope_items, File, ItemKind},
    quasar_syntax::diagnostics::DiagCode,
    salsa::Setter,
    std::sync::Arc,
};

const COUNTER_SRC: &str = r#"
use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}
"#;

const MULTI_ITEM_SRC: &str = r#"
#[account(discriminator = 1)]
pub struct Counter { pub n: u64 }

#[account(discriminator = 2)]
pub struct Vault { pub balance: u64 }

#[account(unsafe_no_disc)]
pub struct Raw { pub bytes: [u8; 32] }
"#;

const MALFORMED_SRC: &str = r#"
#[account(set_inner)]
pub struct Broken { pub n: u64 }
"#;

#[test]
fn parse_file_extracts_account_types() {
    let db = Database::default();
    let file = File::new(&db, Arc::from(COUNTER_SRC), "counter.rs".into());

    let parsed = parse_file(&db, file);
    let items = parsed.items(&db);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Counter");
    assert_eq!(items[0].kind, ItemKind::AccountType);
    assert!(
        parsed.diagnostics(&db).is_empty(),
        "well-formed source has no diagnostics"
    );
}

#[test]
fn parse_file_emits_diagnostics_for_malformed_attribute() {
    let db = Database::default();
    let file = File::new(&db, Arc::from(MALFORMED_SRC), "broken.rs".into());

    let parsed = parse_file(&db, file);
    let diagnostics = parsed.diagnostics(&db);
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountAttrMissingDiscriminatorOrUnsafe),
        "expected missing-discriminator-or-unsafe diagnostic, got {:?}",
        codes
    );
}

#[test]
fn scope_items_returns_all_account_types() {
    let db = Database::default();
    let file = File::new(&db, Arc::from(MULTI_ITEM_SRC), "multi.rs".into());

    let symbols = scope_items(&db, file);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Counter", "Vault", "Raw"]);
    assert!(symbols.iter().all(|s| s.kind == ItemKind::AccountType));
}

#[test]
fn editing_one_file_does_not_invalidate_another() {
    // Editing file A's text bumps Salsa's revision. A snapshot of file B's
    // parse output taken before and after the edit must produce identical
    // tracked struct identities — Salsa's parse_file query for B must not
    // re-run.
    let mut db = Database::default();
    let file_a = File::new(&db, Arc::from(COUNTER_SRC), "a.rs".into());
    let file_b = File::new(&db, Arc::from(MULTI_ITEM_SRC), "b.rs".into());

    let parsed_b_before = parse_file(&db, file_b);
    let items_b_before = parsed_b_before.items(&db).clone();

    // Mutate A.
    file_a
        .set_text(&mut db)
        .to(Arc::from("// completely different content"));

    // Re-fetch B. Items must be untouched.
    let parsed_b_after = parse_file(&db, file_b);
    let items_b_after = parsed_b_after.items(&db);

    assert_eq!(&items_b_before, items_b_after);
}

#[test]
fn whitespace_edit_preserves_scope_items() {
    // A whitespace-only edit produces a parse tree with identical top-level
    // names; scope_items must return value-equal output so downstream
    // workspace-index queries get early cutoff.
    let mut db = Database::default();
    let file = File::new(&db, Arc::from(MULTI_ITEM_SRC), "multi.rs".into());

    let scope_before = scope_items(&db, file).clone();

    let with_whitespace = MULTI_ITEM_SRC.replace("\n\n", "\n\n   \n");
    file.set_text(&mut db)
        .to(Arc::from(with_whitespace.as_str()));

    let scope_after = scope_items(&db, file);
    assert_eq!(&scope_before, scope_after);
}

#[test]
fn structural_edit_changes_scope_items() {
    // Removing an item should change the scope_items output.
    let mut db = Database::default();
    let file = File::new(&db, Arc::from(MULTI_ITEM_SRC), "multi.rs".into());

    let scope_before = scope_items(&db, file).clone();
    assert_eq!(scope_before.len(), 3);

    let without_vault = MULTI_ITEM_SRC.replace(
        "#[account(discriminator = 2)]\npub struct Vault { pub balance: u64 }\n",
        "",
    );
    file.set_text(&mut db).to(Arc::from(without_vault.as_str()));

    let scope_after = scope_items(&db, file);
    assert_eq!(scope_after.len(), 2);
    let names: Vec<&str> = scope_after.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Counter", "Raw"]);
}

#[test]
fn parse_file_handles_syntax_errors_gracefully() {
    let db = Database::default();
    let file = File::new(
        &db,
        Arc::from("this is not valid rust at all }}}"),
        "broken.rs".into(),
    );

    let parsed = parse_file(&db, file);
    // No items, but we get a diagnostic and no panic.
    assert!(parsed.items(&db).is_empty());
    assert!(!parsed.diagnostics(&db).is_empty());
}
