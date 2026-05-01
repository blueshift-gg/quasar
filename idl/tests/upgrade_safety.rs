//! End-to-end integration tests for the upgrade-safety lint family.
//!
//! Exercises the full pipeline: parse → snapshot → persist to lock
//! file → mutate the source → re-parse → diff against the saved
//! snapshot → expect specific L-rule findings. Mirrors how `quasar
//! lint --diff` runs in production but without the CLI / config
//! resolution layer.

use {
    quasar_idl::{
        lint::{
            comparative, preflight,
            snapshot::{ProgramSnapshot, SnapshotIoError, SNAPSHOT_VERSION},
            LintRule,
        },
        parser,
    },
    tempfile::TempDir,
};

const ESCROW_V1: &str = r#"
    declare_id!("11111111111111111111111111111111");

    #[program]
    mod escrow {
        use super::*;

        #[instruction(discriminator = [0])]
        pub fn make(ctx: Ctx<Make>, deposit: u64, receive: u64) -> Result<(), ProgramError> {
            Ok(())
        }

        #[instruction(discriminator = [1])]
        pub fn refund(ctx: Ctx<Refund>) -> Result<(), ProgramError> {
            Ok(())
        }
    }

    #[derive(Accounts)]
    pub struct Make<'info> {
        #[account(mut)]
        pub maker: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct Refund<'info> {
        #[account(mut)]
        pub maker: Signer<'info>,
    }

    #[account(discriminator = [42])]
    pub struct Escrow {
        pub maker: Pubkey,
        pub mint_a: Pubkey,
        pub deposit: u64,
    }
"#;

#[test]
fn snapshot_round_trips_through_lock_file() {
    let parsed = parser::parse_program_from_source(ESCROW_V1);
    let snap = ProgramSnapshot::from_parsed(&parsed);

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("quasar.lock.json");
    snap.save(&path).unwrap();

    let loaded = ProgramSnapshot::load(&path).unwrap();
    assert_eq!(snap, loaded);
    assert_eq!(loaded.version, SNAPSHOT_VERSION);
}

#[test]
fn load_missing_lock_file_is_distinguishable() {
    let dir = TempDir::new().unwrap();
    let err = ProgramSnapshot::load(&dir.path().join("missing.json")).unwrap_err();
    matches!(err, SnapshotIoError::NotFound(_));
}

#[test]
fn diff_against_identical_source_produces_no_findings() {
    let parsed = parser::parse_program_from_source(ESCROW_V1);
    let old = ProgramSnapshot::from_parsed(&parsed);
    let new = ProgramSnapshot::from_parsed(&parsed);
    let diags = comparative::run_all(&old, &new);
    assert!(diags.is_empty(), "unexpected findings: {diags:?}");
}

#[test]
fn removing_instruction_fires_l019() {
    let v1 = parser::parse_program_from_source(ESCROW_V1);
    let v2_src = ESCROW_V1.replace(
        r#"#[instruction(discriminator = [1])]
        pub fn refund(ctx: Ctx<Refund>) -> Result<(), ProgramError> {
            Ok(())
        }"#,
        "",
    );
    let v2 = parser::parse_program_from_source(&v2_src);

    let old = ProgramSnapshot::from_parsed(&v1);
    let new = ProgramSnapshot::from_parsed(&v2);
    let diags = comparative::run_all(&old, &new);
    assert!(diags.iter().any(|d| d.rule == LintRule::L019));
}

#[test]
fn flipping_account_discriminator_fires_l018() {
    let v1 = parser::parse_program_from_source(ESCROW_V1);
    let v2_src = ESCROW_V1.replace("discriminator = [42]", "discriminator = [99]");
    let v2 = parser::parse_program_from_source(&v2_src);

    let old = ProgramSnapshot::from_parsed(&v1);
    let new = ProgramSnapshot::from_parsed(&v2);
    let diags = comparative::run_all(&old, &new);
    assert!(diags.iter().any(|d| d.rule == LintRule::L018));
}

#[test]
fn reordering_account_fields_fires_l013() {
    let v1 = parser::parse_program_from_source(ESCROW_V1);
    let v2_src = ESCROW_V1.replace(
        "pub maker: Pubkey,\n        pub mint_a: Pubkey,\n        pub deposit: u64,",
        "pub deposit: u64,\n        pub maker: Pubkey,\n        pub mint_a: Pubkey,",
    );
    let v2 = parser::parse_program_from_source(&v2_src);

    let old = ProgramSnapshot::from_parsed(&v1);
    let new = ProgramSnapshot::from_parsed(&v2);
    let diags = comparative::run_all(&old, &new);
    assert!(
        diags.iter().any(|d| d.rule == LintRule::L013),
        "expected L013, got {:?}",
        diags.iter().map(|d| d.rule).collect::<Vec<_>>()
    );
}

#[test]
fn preflight_fires_on_escrow_v1_missing_version_and_padding() {
    let parsed = parser::parse_program_from_source(ESCROW_V1);
    let mut diags = Vec::new();
    preflight::run_all(&parsed, &mut diags);
    let rules: Vec<LintRule> = diags.iter().map(|d| d.rule).collect();
    assert!(
        rules.contains(&LintRule::L010),
        "expected L010 in {rules:?}"
    );
    assert!(
        rules.contains(&LintRule::L011),
        "expected L011 in {rules:?}"
    );
}
