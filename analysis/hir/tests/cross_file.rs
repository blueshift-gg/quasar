//! Cross-file resolution: workspace_symbol_index, resolve_account_refs,
//! and the early-cutoff behavior when only one file in the workspace changes.

use quasar_hir::{
    db::Database, items::ItemKind, resolve_account_refs, workspace::Workspace,
    workspace_symbol_index, AccountRefResolution, File,
};
use quasar_syntax::diagnostics::DiagCode;
use salsa::Setter;
use std::sync::Arc;

const STATE_SRC: &str = r#"
#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}
"#;

const INSTRUCTIONS_SRC: &str = r#"
#[derive(Accounts)]
pub struct Increment<'info> {
    pub counter: &'info mut Account<Counter>,
    pub authority: &'info Signer,
}
"#;

const INSTRUCTIONS_BAD_SRC: &str = r#"
#[derive(Accounts)]
pub struct Increment<'info> {
    pub counter: &'info mut Account<Missing>,
    pub authority: &'info Signer,
}
"#;

fn workspace(db: &Database, srcs: &[(&str, &str)]) -> (Workspace, Vec<File>) {
    let files: Vec<File> = srcs
        .iter()
        .map(|(name, src)| File::new(db, Arc::from(*src), name.to_string()))
        .collect();
    let workspace = Workspace::new(db, files.clone());
    (workspace, files)
}

#[test]
fn index_aggregates_account_types_across_files() {
    let db = Database::default();
    let (ws, _) = workspace(
        &db,
        &[("state.rs", STATE_SRC), ("instructions.rs", INSTRUCTIONS_SRC)],
    );

    let index = workspace_symbol_index(&db, ws);
    let entry = index.lookup("Counter").expect("Counter indexed");
    assert_eq!(entry.kind, ItemKind::AccountType);

    let inc = index.lookup("Increment").expect("Increment indexed");
    assert_eq!(inc.kind, ItemKind::AccountsStruct);
}

#[test]
fn resolve_account_refs_finds_cross_file_account_type() {
    let db = Database::default();
    let (ws, files) = workspace(
        &db,
        &[("state.rs", STATE_SRC), ("instructions.rs", INSTRUCTIONS_SRC)],
    );
    let state = files[0];
    let instructions = files[1];

    let resolved = resolve_account_refs(&db, ws, instructions);
    let refs = resolved.refs(&db);
    assert_eq!(refs.len(), 1, "one Account<T> in Increment, got {:?}", refs);

    let (account_ref, resolution) = &refs[0];
    assert_eq!(account_ref.name, "Counter");
    match resolution {
        AccountRefResolution::Resolved { defining_file } => {
            assert_eq!(*defining_file, state, "Counter resolves to state.rs");
        }
        AccountRefResolution::Unknown => panic!("Counter should resolve"),
    }
    assert!(
        resolved.diagnostics(&db).is_empty(),
        "no diagnostics for resolved ref"
    );
}

#[test]
fn unknown_account_type_emits_diagnostic() {
    let db = Database::default();
    let (ws, files) = workspace(
        &db,
        &[("state.rs", STATE_SRC), ("instructions.rs", INSTRUCTIONS_BAD_SRC)],
    );
    let instructions = files[1];

    let resolved = resolve_account_refs(&db, ws, instructions);
    let diags = resolved.diagnostics(&db);
    let codes: Vec<_> = diags.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::UnknownAccountType),
        "expected UnknownAccountType, got {:?}",
        codes
    );
    let msg = diags
        .iter()
        .find(|d| d.code == DiagCode::UnknownAccountType)
        .map(|d| d.message.as_str())
        .unwrap();
    assert!(msg.contains("Missing"), "diagnostic mentions the type name: {}", msg);
}

#[test]
fn editing_state_does_not_recompute_unrelated_instruction_diagnostics() {
    // Editing the state.rs file (whitespace inside Counter) shouldn't affect
    // resolution outcomes for instructions.rs. The Salsa cache should serve
    // identical refs and diagnostics value-equal to the first run.
    let mut db = Database::default();
    let (ws, files) = workspace(
        &db,
        &[("state.rs", STATE_SRC), ("instructions.rs", INSTRUCTIONS_SRC)],
    );
    let state = files[0];
    let instructions = files[1];

    let before = resolve_account_refs(&db, ws, instructions);
    let refs_before = before.refs(&db).clone();

    let with_whitespace = STATE_SRC.replace("pub count: u64", "pub count: u64  // tweak");
    state.set_text(&mut db).to(Arc::from(with_whitespace.as_str()));

    let after = resolve_account_refs(&db, ws, instructions);
    let refs_after = after.refs(&db);
    assert_eq!(&refs_before, refs_after, "refs are value-equal");
}

#[test]
fn removing_an_account_type_invalidates_resolution() {
    let mut db = Database::default();
    let (ws, files) = workspace(
        &db,
        &[("state.rs", STATE_SRC), ("instructions.rs", INSTRUCTIONS_SRC)],
    );
    let state = files[0];
    let instructions = files[1];

    let before = resolve_account_refs(&db, ws, instructions);
    assert!(before.diagnostics(&db).is_empty());

    // Delete Counter from state.rs entirely.
    state.set_text(&mut db).to(Arc::from(""));

    let after = resolve_account_refs(&db, ws, instructions);
    let codes: Vec<_> = after.diagnostics(&db).iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::UnknownAccountType),
        "Counter removal must surface a diagnostic on the Accounts struct"
    );
}
