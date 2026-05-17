use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_cpi::cpi::*,
};

// close attribute with SPL Token.

#[test]
fn close_attr_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "close attr SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_attr_spl_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, wrong_authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "close attr SPL with wrong authority should fail"
    );
}

#[test]
fn close_attr_spl_wrong_mint() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, wrong_mint, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "close attr SPL with wrong mint should fail"
    );
}

// close attribute with Token-2022.

#[test]
fn close_attr_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = CloseTokenT22Instruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "close attr T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_attr_t22_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = CloseTokenT22Instruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, wrong_authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "close attr T22 with wrong authority should fail"
    );
}

// close attribute with InterfaceAccount.

#[test]
fn close_attr_interface_spl_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenInterfaceInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "close attr InterfaceAccount SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_attr_interface_t22_happy() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = CloseTokenInterfaceInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_ok(),
        "close attr InterfaceAccount T22 should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn close_attr_interface_wrong_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let account_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = CloseTokenInterfaceInstruction {
        authority,
        token_account: account_key,
        mint: mint_key,
        destination,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(account_key, mint_key, wrong_authority, 0, token_program),
            mint_account(mint_key, authority, 6, token_program),
            empty_account(destination),
        ],
    );
    assert!(
        result.is_err(),
        "close attr InterfaceAccount wrong authority should fail"
    );
}
