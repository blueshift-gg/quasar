use {
    crate::helpers::{build_simple_data, raw_account, signer_account, svm_misc},
    quasar_svm::{AccountMeta, Instruction, InstructionError, Pubkey},
};

const REWARD: u64 = 5_000_000;

#[test]
fn typed_remaining_group_must_run_close_epilogue() {
    let mut svm = svm_misc();
    let claimant = Pubkey::new_unique();
    let (vault, vault_bump) = Pubkey::find_program_address(
        &[b"simple", quasar_test_misc::EXPECTED_ADDRESS.as_ref()],
        &quasar_test_misc::ID,
    );
    let (receipt, receipt_bump) =
        Pubkey::find_program_address(&[b"simple", claimant.as_ref()], &quasar_test_misc::ID);
    let claimant_start = 1_000_000;
    let vault_start = 20_000_000;
    let receipt_start = 1_000_000;

    assert_eq!(quasar_test_misc::REMAINING_EPILOGUE_FLAGS, (true, false));

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            AccountMeta::new(vault, false),
            AccountMeta::new(claimant, true),
            AccountMeta::new(receipt, false),
        ],
        data: vec![63],
    };
    let first = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                vault,
                vault_start,
                build_simple_data(quasar_test_misc::EXPECTED_ADDRESS, REWARD, vault_bump),
                quasar_test_misc::ID,
            ),
            signer_account(claimant),
            raw_account(
                receipt,
                receipt_start,
                build_simple_data(claimant, 0, receipt_bump),
                quasar_test_misc::ID,
            ),
        ],
    );
    assert_eq!(first.raw_result, Ok(()), "the initial claim must succeed");

    let replay = svm.process_instruction(&instruction, &[]);
    let receipt_after_replay = replay.account(&receipt).expect("receipt account");

    assert_eq!(
        (
            replay.raw_result.clone(),
            replay.account(&claimant).unwrap().lamports,
            replay.account(&vault).unwrap().lamports,
            receipt_after_replay.lamports,
            receipt_after_replay.owner,
            receipt_after_replay.data.len(),
        ),
        (
            Err(InstructionError::IllegalOwner),
            claimant_start + REWARD + receipt_start,
            vault_start - REWARD,
            0,
            quasar_svm::system_program::ID,
            0,
        ),
        "the remaining-group close epilogue must consume the receipt before replay",
    );
}
