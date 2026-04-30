use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_migrate::cpi::*,
};

// ---------------------------------------------------------------------------
// Raw data builders
// ---------------------------------------------------------------------------

/// ConfigV1: disc=1, authority: Address (32), value: PodU64 (8)
/// Total = 1 + 32 + 8 = 41 bytes
const CONFIG_V1_SIZE: usize = 41;

fn build_config_v1_data(authority: Pubkey, value: u64) -> Vec<u8> {
    let mut data = vec![0u8; CONFIG_V1_SIZE];
    data[0] = 1; // discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data
}

/// ConfigV2: disc=2, authority: Address (32), value: PodU64 (8), extra: PodU32
/// (4) Total = 1 + 32 + 8 + 4 = 45 bytes
const CONFIG_V2_SIZE: usize = 45;

// ---------------------------------------------------------------------------
// SVM factory
// ---------------------------------------------------------------------------

fn svm_migrate() -> quasar_svm::QuasarSvm {
    let path = "../../target/deploy/quasar_test_migrate.so";
    let elf = std::fs::read(path)
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}. Run `make build-sbf` first."));
    quasar_svm::QuasarSvm::new().with_program(&quasar_test_migrate::ID, &elf)
}

fn config_v1_account(address: Pubkey, authority: Pubkey, value: u64) -> quasar_svm::Account {
    raw_account(
        address,
        1_000_000,
        build_config_v1_data(authority, value),
        quasar_test_migrate::ID,
    )
}

// ============================================================================
// Happy path: V1 → V2 migration
// ============================================================================

#[test]
fn migrate_v1_to_v2() {
    let mut svm = svm_migrate();
    let payer = Pubkey::new_unique();
    let config = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = MigrateConfigInstruction {
        payer,
        system_program: quasar_svm::system_program::ID,
        config,
        authority,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            // system_program is auto-provided by QuasarSvm — no account entry needed
            config_v1_account(config, authority, 100),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "migrate V1→V2 failed: {:?}",
        result.raw_result
    );

    // Verify the account was migrated: disc should be 2, size should be V2
    let migrated = result.account(&config).expect("config account exists");
    assert_eq!(migrated.data.len(), CONFIG_V2_SIZE, "account resized to V2");
    assert_eq!(migrated.data[0], 2, "discriminator updated to V2");

    // authority field preserved
    assert_eq!(
        &migrated.data[1..33],
        authority.as_ref(),
        "authority preserved"
    );
    // value field preserved
    assert_eq!(
        u64::from_le_bytes(migrated.data[33..41].try_into().unwrap()),
        100,
        "value preserved"
    );
    // extra field set by migration logic
    assert_eq!(
        u32::from_le_bytes(migrated.data[41..45].try_into().unwrap()),
        42,
        "extra field set to 42 by migrate()"
    );
}

// ============================================================================
// Error: wrong authority (has_one fails)
// ============================================================================

#[test]
fn migrate_wrong_authority_fails() {
    let mut svm = svm_migrate();
    let payer = Pubkey::new_unique();
    let config = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();

    let ix: Instruction = MigrateConfigInstruction {
        payer,
        system_program: quasar_svm::system_program::ID,
        config,
        authority: wrong_authority,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            config_v1_account(config, authority, 100), // authority != wrong_authority
            signer_account(wrong_authority),
        ],
    );
    assert!(result.is_err(), "wrong authority should fail");
}
