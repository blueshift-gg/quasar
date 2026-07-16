use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_cpi::cpi::*,
};

#[test]
fn close_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenAccountInstruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "close SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = CloseTokenAccountT22Instruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "close Token-2022 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenAccountInterfaceInstruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "close interface SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_interface_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = CloseTokenAccountInterfaceInstruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "close interface Token-2022 should succeed: {:?}",
        result.raw_result
    );
}

// Error propagation through the CPI machinery: the SPL program's own
// rejection must surface exactly, not be masked or remapped. SPL close
// checks balance before owner, so the two cases are distinguishable.

#[test]
fn close_rejects_non_zero_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenAccountInstruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 100, token_program),
        ],
    );
    // spl_token::TokenError::NonNativeHasBalance = 11
    result.assert_error(quasar_svm::ProgramError::Custom(11));
}

#[test]
fn close_rejects_wrong_owner() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_owner = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenAccountInstruction {
        authority,
        account: account_key,
        destination: authority,
        token_program,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            // Zero balance passes the first gate; validate_owner then fails.
            token_account(account_key, mint_key, wrong_owner, 0, token_program),
        ],
    );
    // spl_token::TokenError::OwnerMismatch = 4
    result.assert_error(quasar_svm::ProgramError::Custom(4));
}
