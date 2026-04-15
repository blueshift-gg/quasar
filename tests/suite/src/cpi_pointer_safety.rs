use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_misc::cpi::*,
};

// ============================================================================
// CPI pointer safety: mutable data write + CPI on the same account
//
// The handler writes to Account<SimpleAccount> data via set_inner() (raw
// pointer write through data_mut_ptr, no borrow tracking), then passes
// the SAME account into a system transfer CPI as the writable destination.
// cpi_account_from_view() extracts raw pointers without checking
// borrow_state.
//
// Verifies:
//   - set_inner data write survives the CPI round-trip (SVM
//     serialize -> execute -> deserialize doesn't clobber it)
//   - CPI lamport change is visible through the same AccountView
//   - A second set_inner after CPI still writes correctly
//     (data_mut_ptr still valid)
// ============================================================================

#[test]
fn mut_readback_data_and_lamports() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = CpiMutReadbackInstruction {
        account,
        payer,
        system_program: quasar_svm::system_program::ID,
        new_value: 999,
    }
    .into();

    let initial_lamports = 1_000_000u64;
    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, payer, 42, 0),
            rich_signer_account(payer),
        ],
    );
    assert!(result.is_ok(), "mut readback: {:?}", result.raw_result);

    // Off-chain: verify data reflects the SECOND set_inner (value = 999 + 1 = 1000)
    let acc = result.account(&account).expect("account");
    let final_value = u64::from_le_bytes(acc.data[33..41].try_into().unwrap());
    assert_eq!(final_value, 1000, "second set_inner value");

    // Off-chain: verify lamports include the 1000 from CPI transfer
    assert_eq!(acc.lamports, initial_lamports + 1_000, "lamports after CPI");
}

#[test]
fn mut_readback_wrapping_overflow() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = CpiMutReadbackInstruction {
        account,
        payer,
        system_program: quasar_svm::system_program::ID,
        new_value: u64::MAX,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, payer, 42, 0),
            rich_signer_account(payer),
        ],
    );
    assert!(result.is_ok(), "mut max value: {:?}", result.raw_result);

    // u64::MAX wrapping_add(1) == 0
    let acc = result.account(&account).expect("account");
    let final_value = u64::from_le_bytes(acc.data[33..41].try_into().unwrap());
    assert_eq!(final_value, 0, "wrapping add overflow");
}

#[test]
fn mut_readback_zero_value() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = CpiMutReadbackInstruction {
        account,
        payer,
        system_program: quasar_svm::system_program::ID,
        new_value: 0,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, payer, 42, 0),
            rich_signer_account(payer),
        ],
    );
    assert!(result.is_ok(), "mut zero: {:?}", result.raw_result);
}
