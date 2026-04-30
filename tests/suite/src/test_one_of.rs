use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_one_of::cpi::*,
};

// ---------------------------------------------------------------------------
// Raw data builders
// ---------------------------------------------------------------------------

/// Settings: disc=10, authority: Address (32), threshold: PodU16 (2)
/// Total = 1 + 32 + 2 = 35 bytes
const SETTINGS_SIZE: usize = 35;

fn build_settings_data(authority: Pubkey, threshold: u16) -> Vec<u8> {
    let mut data = vec![0u8; SETTINGS_SIZE];
    data[0] = 10; // discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..35].copy_from_slice(&threshold.to_le_bytes());
    data
}

/// Policy: disc=11, authority: Address (32), max_amount: PodU64 (8), threshold:
/// PodU16 (2) Total = 1 + 32 + 8 + 2 = 43 bytes
const POLICY_SIZE: usize = 43;

fn build_policy_data(authority: Pubkey, max_amount: u64, threshold: u16) -> Vec<u8> {
    let mut data = vec![0u8; POLICY_SIZE];
    data[0] = 11; // discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&max_amount.to_le_bytes());
    data[41..43].copy_from_slice(&threshold.to_le_bytes());
    data
}

// ---------------------------------------------------------------------------
// SVM factory
// ---------------------------------------------------------------------------

fn svm_one_of() -> quasar_svm::QuasarSvm {
    let path = "../../target/deploy/quasar_test_one_of.so";
    let elf = std::fs::read(path)
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}. Run `make build-sbf` first."));
    quasar_svm::QuasarSvm::new().with_program(&quasar_test_one_of::ID, &elf)
}

fn consensus_account(address: Pubkey, data: Vec<u8>) -> quasar_svm::Account {
    raw_account(address, 1_000_000, data, quasar_test_one_of::ID)
}

// ============================================================================
// Happy path: Settings variant
// ============================================================================

#[test]
fn load_settings_via_one_of() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    let data = build_settings_data(signer, 100);
    let ix: Instruction = CheckConsensusInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[signer_account(signer), consensus_account(target, data)],
    );
    assert!(
        result.is_ok(),
        "settings via one_of: {:?}",
        result.raw_result
    );
}

// ============================================================================
// Happy path: Policy variant
// ============================================================================

#[test]
fn load_policy_via_one_of() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    let data = build_policy_data(signer, 1_000_000, 200);
    let ix: Instruction = CheckConsensusInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[signer_account(signer), consensus_account(target, data)],
    );
    assert!(result.is_ok(), "policy via one_of: {:?}", result.raw_result);
}

// ============================================================================
// Typed accessor via variant()
// ============================================================================

#[test]
fn typed_accessor_settings() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    let data = build_settings_data(signer, 50);
    let ix: Instruction = TypedAccessorInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[signer_account(signer), consensus_account(target, data)],
    );
    assert!(
        result.is_ok(),
        "typed accessor settings: {:?}",
        result.raw_result
    );
}

#[test]
fn typed_accessor_policy() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    let data = build_policy_data(signer, 500_000, 75);
    let ix: Instruction = TypedAccessorInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[signer_account(signer), consensus_account(target, data)],
    );
    assert!(
        result.is_ok(),
        "typed accessor policy: {:?}",
        result.raw_result
    );
}

// ============================================================================
// Rejection: wrong discriminator
// ============================================================================

#[test]
fn rejects_unknown_discriminator() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    // Discriminator 99 matches neither Settings (10) nor Policy (11)
    let mut data = vec![0u8; SETTINGS_SIZE];
    data[0] = 99;
    let ix: Instruction = CheckConsensusInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[signer_account(signer), consensus_account(target, data)],
    );
    assert!(
        result.raw_result.is_err(),
        "unknown discriminator should be rejected"
    );
}

// ============================================================================
// Rejection: wrong owner
// ============================================================================

#[test]
fn rejects_wrong_owner() {
    let mut svm = svm_one_of();
    let signer = Pubkey::new_unique();
    let target = Pubkey::new_unique();
    let data = build_settings_data(signer, 100);
    // Account owned by system program, not our program
    let bad_account = raw_account(target, 1_000_000, data, Pubkey::default());
    let ix: Instruction = CheckConsensusInstruction {
        signer,
        consensus: target,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer), bad_account]);
    assert!(result.raw_result.is_err(), "wrong owner should be rejected");
}
