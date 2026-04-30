use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_validate::cpi::*,
};

// ===========================================================================
// Account<Token> (SPL Token) — ValidateTokenCheck
// ===========================================================================

#[test]
fn account_token_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn account_token_wrong_mint() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, wrong_mint, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: mint mismatch");
}

#[test]
fn account_token_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn account_token_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    // Valid token data but Account.owner set to wrong program
    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, authority, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn account_token_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 165], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized token account");
}

#[test]
fn account_token_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 10], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// Account<Token2022> — ValidateToken2022Check
// ===========================================================================

#[test]
fn account_token2022_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn account_token2022_wrong_mint() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, wrong_mint, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: mint mismatch");
}

#[test]
fn account_token2022_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn account_token2022_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, authority, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn account_token2022_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 165], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized token account");
}

#[test]
fn account_token2022_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateToken2022CheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 10], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// InterfaceAccount<Token> with SPL Token — ValidateTokenInterfaceCheck
// ===========================================================================

#[test]
fn interface_token_spl_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn interface_token_spl_wrong_mint() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, wrong_mint, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: mint mismatch");
}

#[test]
fn interface_token_spl_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn interface_token_spl_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, authority, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn interface_token_spl_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 165], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized token account");
}

#[test]
fn interface_token_spl_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 10], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// InterfaceAccount<Token> with Token-2022 — ValidateTokenInterfaceCheck
// ===========================================================================

#[test]
fn interface_token_t22_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn interface_token_t22_wrong_mint() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, wrong_mint, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: mint mismatch");
}

#[test]
fn interface_token_t22_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn interface_token_t22_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, authority, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn interface_token_t22_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 165], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized token account");
}

#[test]
fn interface_token_t22_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(token_key, 1_000_000, vec![0u8; 10], token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// InterfaceAccount cross-program mismatch
// ===========================================================================

#[test]
fn interface_token_cross_program_mismatch() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let actual_owner = token_2022_program_id();
    let wrong_program = spl_token_program_id();

    // Token account is owned by token_2022 but we pass spl_token as token_program
    let instruction: Instruction = ValidateTokenInterfaceCheckInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program: wrong_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, actual_owner),
            mint_account(mint_key, authority, 6, actual_owner),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: cross-program mismatch");
}

// ===========================================================================
// No token_program field — ValidateTokenNoProgram
// ===========================================================================

#[test]
fn no_program_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenNoProgramInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn no_program_wrong_mint() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenNoProgramInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, wrong_mint, authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: mint mismatch");
}

#[test]
fn no_program_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateTokenNoProgramInstruction {
        token_account: token_key,
        mint: mint_key,
        authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}
