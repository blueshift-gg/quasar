//! `define_account!` account types are first-class: they get symbol-index
//! entries (completion / goto / hover) and, when their data struct is declared
//! in the same file, full `has_one` field validation. Types whose data struct
//! can't be resolved fall back to "fields unknown" rather than false-flagging.

use {
    quasar_hir::{
        db::Database, parse_file, resolve_account_refs, resolve_has_one, workspace::Workspace,
        workspace_symbol_index, AccountRefResolution, File, HasOneResolution, ItemKind,
    },
    quasar_syntax::diagnostics::DiagCode,
    std::sync::Arc,
};

fn workspace(db: &Database, srcs: &[(&str, &str)]) -> (Workspace, Vec<File>) {
    let files: Vec<File> = srcs
        .iter()
        .map(|(name, src)| File::new(db, Arc::from(*src), name.to_string()))
        .collect();
    (Workspace::new(db, files.clone(), Vec::new()), files)
}

// SPL-style: a `define_account!` wrapper plus its data struct, in one file.
const SPL: &str = r#"
quasar_lang::define_account!(
    /// Token account data.
    pub struct Token => [checks::ZeroPod]: TokenData
);

pub struct TokenData {
    pub mint: Address,
    pub owner: Address,
    pub amount: u64,
}

quasar_lang::define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);
"#;

#[test]
fn define_account_type_is_indexed_and_resolves() {
    let db = Database::default();
    let ix = r#"
#[derive(Accounts)]
pub struct Ix {
    pub token: Account<Token>,
}
"#;
    let (ws, files) = workspace(&db, &[("spl.rs", SPL), ("ix.rs", ix)]);

    let index = workspace_symbol_index(&db, ws);
    let token = index.lookup("Token").expect("Token indexed");
    assert_eq!(token.kind, ItemKind::AccountType);
    assert_eq!(token.file, files[0], "Token resolves to spl.rs");
    assert!(
        index.lookup("TokenProgram").is_some(),
        "marker also indexed"
    );

    // `Account<Token>` resolves to the defining file (enables goto/hover).
    let resolved = resolve_account_refs(&db, ws, files[1]);
    let (r, res) = &resolved.refs(&db)[0];
    assert_eq!(r.name, "Token");
    assert_eq!(
        *res,
        AccountRefResolution::Resolved {
            defining_file: files[0]
        }
    );
}

#[test]
fn define_account_range_points_at_the_name() {
    let db = Database::default();
    let (_ws, files) = workspace(&db, &[("spl.rs", SPL)]);
    let parsed = parse_file(&db, files[0]);
    let token = parsed
        .items(&db)
        .iter()
        .find(|i| i.name == "Token")
        .expect("Token item");
    assert_eq!(token.kind, ItemKind::AccountType);
    // The range must select the `Token` identifier so goto lands on it.
    let slice = &SPL[token.range.start as usize..token.range.end as usize];
    assert_eq!(slice, "Token");
}

#[test]
fn define_account_has_one_validates_resolved_data_fields() {
    let db = Database::default();
    // `owner` IS a TokenData field -> resolves; `bogus` is not -> missing.
    let ix = r#"
#[derive(Accounts)]
pub struct Ix {
    pub owner: Signer,
    pub admin: Signer,
    #[account(has_one(owner))]
    pub token: Account<Token>,
    #[account(has_one(admin))]
    pub other: Account<Token>,
}
"#;
    let (ws, files) = workspace(&db, &[("spl.rs", SPL), ("ix.rs", ix)]);
    let resolved = resolve_has_one(&db, ws, files[1]);

    let owner = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "owner")
        .cloned()
        .unwrap();
    assert_eq!(owner.resolution, HasOneResolution::Resolved);

    let admin = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "admin")
        .cloned()
        .unwrap();
    assert_eq!(
        admin.resolution,
        HasOneResolution::MissingAccountField {
            account_type: "Token".to_string()
        }
    );
}

#[test]
fn define_account_resolves_data_struct_from_another_file() {
    let db = Database::default();
    // The `define_account!` wrapper and its data struct live in SEPARATE files
    // — the general case for a dependency crate. has_one must still validate.
    let wrapper = "quasar_lang::define_account!(pub struct Token => [checks::ZeroPod]: TokenData);";
    let data = "pub struct TokenData { pub mint: Address, pub owner: Address }";
    let ix = r#"
#[derive(Accounts)]
pub struct Ix {
    pub owner: Signer,
    pub admin: Signer,
    #[account(has_one(owner))]
    pub token: Account<Token>,
    #[account(has_one(admin))]
    pub other: Account<Token>,
}
"#;
    let (ws, files) = workspace(
        &db,
        &[("wrapper.rs", wrapper), ("data.rs", data), ("ix.rs", ix)],
    );
    let resolved = resolve_has_one(&db, ws, files[2]);

    let owner = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "owner")
        .cloned()
        .unwrap();
    assert_eq!(
        owner.resolution,
        HasOneResolution::Resolved,
        "owner is a TokenData field resolved from another file"
    );
    let admin = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "admin")
        .cloned()
        .unwrap();
    assert_eq!(
        admin.resolution,
        HasOneResolution::MissingAccountField {
            account_type: "Token".to_string()
        },
        "admin is not a TokenData field"
    );
}

#[test]
fn define_account_without_resolvable_data_does_not_false_flag() {
    let db = Database::default();
    // The data struct (`TokenData`) is NOT present anywhere, so fields are
    // unknown — `has_one` must not flag a missing field.
    let spl = r#"
quasar_lang::define_account!(pub struct Token => [checks::ZeroPod]: TokenData);
"#;
    let ix = r#"
#[derive(Accounts)]
pub struct Ix {
    pub owner: Signer,
    #[account(has_one(owner))]
    pub token: Account<Token>,
}
"#;
    let (ws, files) = workspace(&db, &[("spl.rs", spl), ("ix.rs", ix)]);
    let resolved = resolve_has_one(&db, ws, files[1]);
    let codes: Vec<_> = resolved.diagnostics(&db).iter().map(|d| d.code).collect();
    assert!(
        !codes.contains(&DiagCode::HasOneMissingAccountField),
        "fields unknown -> no missing-field diagnostic, got {:?}",
        codes
    );
    let owner = resolved
        .refs(&db)
        .iter()
        .find(|r| r.target == "owner")
        .cloned()
        .unwrap();
    assert_eq!(owner.resolution, HasOneResolution::Resolved);
}
