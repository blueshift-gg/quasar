use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_misc::cpi::*,
};

fn program_id() -> Pubkey {
    quasar_test_misc::ID
}

/// Build raw VaultV1 data: disc=20, authority(32), value(8) = 41 bytes
fn vault_v1_data(authority: Pubkey, value: u64) -> Vec<u8> {
    let mut data = vec![0u8; 41];
    data[0] = 20; // discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data
}

/// Build raw VaultV2 data: disc=21, authority(32), value(8), fee(8) = 49 bytes
fn vault_v2_data(authority: Pubkey, value: u64, fee: u64) -> Vec<u8> {
    let mut data = vec![0u8; 49];
    data[0] = 21; // discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data[41..49].copy_from_slice(&fee.to_le_bytes());
    data
}

// Happy paths for both V1 and V2 through InterfaceAccount<VaultInterface>.

#[test]
fn vault_v1_accepted() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            vault,
            1_000_000,
            vault_v1_data(authority, 100),
            program_id(),
        )],
    );
    assert!(
        result.is_ok(),
        "VaultV1 should be accepted: {:?}",
        result.raw_result
    );
}

#[test]
fn vault_v2_accepted() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            vault,
            1_000_000,
            vault_v2_data(authority, 100, 50),
            program_id(),
        )],
    );
    assert!(
        result.is_ok(),
        "VaultV2 should be accepted: {:?}",
        result.raw_result
    );
}

// Error paths.

#[test]
fn wrong_owner_rejected() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let wrong_owner = Pubkey::new_unique();
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            vault,
            1_000_000,
            vault_v1_data(authority, 100),
            wrong_owner,
        )],
    );
    assert!(result.is_err(), "wrong owner should be rejected");
}

#[test]
fn wrong_discriminator_rejected() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    let mut data = vec![0u8; 49];
    data[0] = 99; // neither 20 nor 21
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(&ix, &[raw_account(vault, 1_000_000, data, program_id())]);
    assert!(result.is_err(), "unknown discriminator should be rejected");
}

#[test]
fn v1_data_too_small_rejected() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    let mut data = vec![0u8; 20]; // too small for VaultV1 (needs 41)
    data[0] = 20;
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(&ix, &[raw_account(vault, 1_000_000, data, program_id())]);
    assert!(result.is_err(), "undersized VaultV1 should be rejected");
}

#[test]
fn v2_data_too_small_rejected() {
    let mut svm = svm_misc();
    let vault = Pubkey::new_unique();
    // 41 bytes with disc=21 is enough for V1 but not V2.
    let mut data = vec![0u8; 41];
    data[0] = 21;
    let ix: Instruction = InterfaceMigrationCheckInstruction { vault }.into();
    let result = svm.process_instruction(&ix, &[raw_account(vault, 1_000_000, data, program_id())]);
    assert!(result.is_err(), "undersized VaultV2 should be rejected");
}
