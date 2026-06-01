//! Constraint validation: structural rules from the derive surfaced as
//! diagnostics.

use {
    quasar_hir::{db::Database, validate_accounts, File},
    quasar_syntax::diagnostics::DiagCode,
    std::sync::Arc,
};

fn diagnostics(src: &str) -> Vec<DiagCode> {
    let db = Database::default();
    let file = File::new(&db, Arc::from(src), "ix.rs".to_string());
    validate_accounts(&db, file)
        .diagnostics(&db)
        .iter()
        .map(|d| d.code)
        .collect()
}

fn messages(src: &str) -> Vec<String> {
    let db = Database::default();
    let file = File::new(&db, Arc::from(src), "ix.rs".to_string());
    validate_accounts(&db, file)
        .diagnostics(&db)
        .iter()
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn init_realloc_conflict_is_flagged() {
    // `init` and `realloc` are mutually exclusive.
    let src = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: Signer,
    #[account(init, realloc = 128, payer = payer)]
    pub account: &'info mut Account<Thing>,
}
"#;
    let codes = diagnostics(src);
    assert!(
        codes.contains(&DiagCode::AccountsConstraintViolation),
        "init + realloc should be flagged, got {:?}",
        codes
    );
}

#[test]
fn realloc_without_mut_reports_requires_mut_message() {
    // realloc (unlike init) does not imply mut; omitting it is an error whose
    // message drives the "Add `mut`" code action.
    let src = r#"
#[derive(Accounts)]
pub struct Resize<'info> {
    pub payer: Signer,
    #[account(realloc = 128, payer = payer)]
    pub account: &'info Account<Thing>,
}
"#;
    assert!(
        messages(src).iter().any(|m| m.contains("requires `mut`")),
        "message should mention the missing mut, got {:?}",
        messages(src)
    );
}

#[test]
fn realloc_without_mut_is_flagged() {
    let src = r#"
#[derive(Accounts)]
pub struct Resize<'info> {
    pub payer: Signer,
    #[account(realloc = 128, payer = payer)]
    pub account: &'info Account<Thing>,
}
"#;
    assert!(
        diagnostics(src).contains(&DiagCode::AccountsConstraintViolation),
        "realloc without mut should be flagged"
    );
}

#[test]
fn well_formed_accounts_struct_is_clean() {
    let src = r#"
#[derive(Accounts)]
pub struct Increment<'info> {
    pub authority: Signer,
    #[account(mut)]
    pub counter: &'info mut Account<Counter>,
}
"#;
    assert!(
        diagnostics(src).is_empty(),
        "well-formed struct should have no constraint diagnostics, got {:?}",
        diagnostics(src)
    );
}

#[test]
fn non_accounts_struct_is_ignored() {
    let src = r#"
#[account(discriminator = 1)]
pub struct Counter {
    pub authority: Address,
    pub count: u64,
}
"#;
    assert!(
        diagnostics(src).is_empty(),
        "plain account types have no Accounts-constraint validation"
    );
}

#[test]
fn syntactically_broken_source_does_not_panic() {
    let src = "this is not valid rust @#$%";
    let _ = diagnostics(src);
}
