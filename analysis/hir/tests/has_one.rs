//! has_one resolution: sibling-binding + account-type-field checks.

use {
    quasar_hir::{db::Database, resolve_has_one, workspace::Workspace, File, HasOneResolution},
    quasar_syntax::diagnostics::DiagCode,
    salsa::Setter,
    std::sync::Arc,
};

fn workspace(db: &Database, srcs: &[(&str, &str)]) -> (Workspace, Vec<File>) {
    let files: Vec<File> = srcs
        .iter()
        .map(|(name, src)| File::new(db, Arc::from(*src), name.to_string()))
        .collect();
    (Workspace::new(db, files.clone(), Vec::new()), files)
}

const VAULT: &str = r#"
#[account(discriminator = 3)]
pub struct Vault {
    pub authority: Address,
    pub amount: u64,
}
"#;

#[test]
fn has_one_resolves_when_binding_and_field_exist() {
    let db = Database::default();
    let accounts = r#"
#[derive(Accounts)]
pub struct CheckAccounts {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub vault: Account<Vault>,
}
"#;
    let (ws, files) = workspace(&db, &[("state.rs", VAULT), ("ix.rs", accounts)]);
    let resolved = resolve_has_one(&db, ws, files[1]);
    assert!(
        resolved.diagnostics(&db).is_empty(),
        "well-formed has_one should not produce diagnostics: {:?}",
        resolved.diagnostics(&db)
    );
    let refs = resolved.refs(&db);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].target, "authority");
    assert_eq!(refs[0].binding, "vault");
    assert_eq!(refs[0].account_type.as_deref(), Some("Vault"));
    assert_eq!(refs[0].resolution, HasOneResolution::Resolved);
}

#[test]
fn has_one_unknown_binding_is_diagnosed() {
    let db = Database::default();
    let accounts = r#"
#[derive(Accounts)]
pub struct CheckAccounts {
    #[account(has_one(manager))]
    pub vault: Account<Vault>,
}
"#;
    let (ws, files) = workspace(&db, &[("state.rs", VAULT), ("ix.rs", accounts)]);
    let resolved = resolve_has_one(&db, ws, files[1]);
    let codes: Vec<_> = resolved.diagnostics(&db).iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::HasOneUnknownBinding),
        "expected unknown-binding diagnostic, got {:?}",
        codes
    );
}

#[test]
fn has_one_missing_account_field_is_diagnosed() {
    let db = Database::default();
    // Vault has no `manager` field, but there IS a sibling binding `manager`.
    let accounts = r#"
#[derive(Accounts)]
pub struct CheckAccounts {
    pub manager: Signer,
    #[account(has_one(manager))]
    pub vault: Account<Vault>,
}
"#;
    let (ws, files) = workspace(&db, &[("state.rs", VAULT), ("ix.rs", accounts)]);
    let resolved = resolve_has_one(&db, ws, files[1]);
    let codes: Vec<_> = resolved.diagnostics(&db).iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::HasOneMissingAccountField),
        "expected missing-account-field diagnostic, got {:?}",
        codes
    );
    let r = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "manager")
        .cloned()
        .unwrap();
    assert_eq!(
        r.resolution,
        HasOneResolution::MissingAccountField {
            account_type: "Vault".to_string()
        }
    );
}

#[test]
fn has_one_with_unresolved_account_type_does_not_double_flag() {
    let db = Database::default();
    // `Account<Missing>` — the account type isn't in the workspace. has_one
    // resolution should not emit a missing-field diagnostic (the unknown
    // account type is the account resolver's concern).
    let accounts = r#"
#[derive(Accounts)]
pub struct CheckAccounts {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub vault: Account<Missing>,
}
"#;
    let (ws, files) = workspace(&db, &[("ix.rs", accounts)]);
    let resolved = resolve_has_one(&db, ws, files[0]);
    let codes: Vec<_> = resolved.diagnostics(&db).iter().map(|d| d.code).collect();
    assert!(
        !codes.contains(&DiagCode::HasOneMissingAccountField),
        "should not flag missing field when account type is unknown, got {:?}",
        codes
    );
}

#[test]
fn editing_account_type_updates_has_one_resolution() {
    let mut db = Database::default();
    let accounts = r#"
#[derive(Accounts)]
pub struct CheckAccounts {
    pub manager: Signer,
    #[account(has_one(manager))]
    pub vault: Account<Vault>,
}
"#;
    let (ws, files) = workspace(&db, &[("state.rs", VAULT), ("ix.rs", accounts)]);
    let state = files[0];

    // Initially Vault lacks `manager` -> missing-field diagnostic.
    let before = resolve_has_one(&db, ws, files[1]);
    assert!(before
        .diagnostics(&db)
        .iter()
        .any(|d| d.code == DiagCode::HasOneMissingAccountField));

    // Add the `manager` field to Vault.
    let fixed = r#"
#[account(discriminator = 3)]
pub struct Vault {
    pub authority: Address,
    pub manager: Address,
    pub amount: u64,
}
"#;
    state.set_text(&mut db).to(Arc::from(fixed));

    let after = resolve_has_one(&db, ws, files[1]);
    assert!(
        !after
            .diagnostics(&db)
            .iter()
            .any(|d| d.code == DiagCode::HasOneMissingAccountField),
        "adding the field should clear the diagnostic"
    );
}
