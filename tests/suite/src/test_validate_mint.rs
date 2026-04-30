use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_validate::cpi::*,
};

// ===========================================================================
// Account<Mint> (SPL Token) — ValidateMintCheck
// ===========================================================================

#[test]
fn mint_spl_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_spl_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn mint_spl_wrong_decimals() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: decimals mismatch (9 != 6)");
}

#[test]
fn mint_spl_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                mint_key,
                1_000_000,
                pack_mint_data(authority, 6),
                Pubkey::default(),
            ),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn mint_spl_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 82], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized mint account");
}

#[test]
fn mint_spl_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 10], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// Account<Mint2022> (Token-2022) — ValidateMint2022Check
// ===========================================================================

#[test]
fn mint_t22_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_t22_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn mint_t22_wrong_decimals() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: decimals mismatch (9 != 6)");
}

#[test]
fn mint_t22_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                mint_key,
                1_000_000,
                pack_mint_data(authority, 6),
                Pubkey::default(),
            ),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn mint_t22_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 82], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized mint account");
}

#[test]
fn mint_t22_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMint2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 10], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// InterfaceAccount<Mint> with SPL Token — ValidateMintInterfaceCheck
// ===========================================================================

#[test]
fn mint_interface_spl_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_interface_spl_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn mint_interface_spl_wrong_decimals() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: decimals mismatch (9 != 6)");
}

#[test]
fn mint_interface_spl_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                mint_key,
                1_000_000,
                pack_mint_data(authority, 6),
                Pubkey::default(),
            ),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn mint_interface_spl_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 82], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized mint account");
}

#[test]
fn mint_interface_spl_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 10], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// InterfaceAccount<Mint> with Token-2022 — ValidateMintInterfaceCheck
// ===========================================================================

#[test]
fn mint_interface_t22_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_interface_t22_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn mint_interface_t22_wrong_decimals() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: decimals mismatch (9 != 6)");
}

#[test]
fn mint_interface_t22_wrong_owner() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                mint_key,
                1_000_000,
                pack_mint_data(authority, 6),
                Pubkey::default(),
            ),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: account owner is wrong program"
    );
}

#[test]
fn mint_interface_t22_uninitialized() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 82], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: uninitialized mint account");
}

#[test]
fn mint_interface_t22_data_too_small() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(mint_key, 1_000_000, vec![0u8; 10], token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: data too small");
}

// ===========================================================================
// No token_program field — ValidateMintNoProgram
// ===========================================================================

#[test]
fn mint_no_program_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintNoProgramInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_no_program_wrong_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintNoProgramInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: authority mismatch");
}

#[test]
fn mint_no_program_wrong_decimals() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintNoProgramInstruction {
        mint: mint_key,
        mint_authority: authority,
        token_program: quasar_spl::SPL_TOKEN_ID,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(result.is_err(), "should fail: decimals mismatch (9 != 6)");
}

// ===========================================================================
// Freeze authority — ValidateMintWithFreezeCheck (SPL Token)
// ===========================================================================

#[test]
fn mint_spl_freeze_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_spl_freeze_wrong_freeze_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_err(), "should fail: freeze authority mismatch");
}

#[test]
fn mint_spl_freeze_missing_on_chain() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    // On-chain mint has no freeze_authority, but handler expects one
    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: on-chain mint has no freeze authority"
    );
}

// ===========================================================================
// Freeze authority — ValidateMintWithFreeze2022Check (Token-2022)
// ===========================================================================

#[test]
fn mint_t22_freeze_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreeze2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_t22_freeze_wrong_freeze_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreeze2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_err(), "should fail: freeze authority mismatch");
}

#[test]
fn mint_t22_freeze_missing_on_chain() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreeze2022CheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    // On-chain mint has no freeze_authority, but handler expects one
    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: on-chain mint has no freeze authority"
    );
}

// ===========================================================================
// Freeze authority — ValidateMintWithFreezeInterfaceCheck (SPL via Interface)
// ===========================================================================

#[test]
fn mint_interface_spl_freeze_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_interface_spl_freeze_wrong_freeze_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_err(), "should fail: freeze authority mismatch");
}

#[test]
fn mint_interface_spl_freeze_missing_on_chain() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    // On-chain mint has no freeze_authority, but handler expects one
    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: on-chain mint has no freeze authority"
    );
}

// ===========================================================================
// Freeze authority — ValidateMintWithFreezeInterfaceCheck (T22 via Interface)
// ===========================================================================

#[test]
fn mint_interface_t22_freeze_happy() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn mint_interface_t22_freeze_wrong_freeze_authority() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(result.is_err(), "should fail: freeze authority mismatch");
}

#[test]
fn mint_interface_t22_freeze_missing_on_chain() {
    let mut svm = svm_validate();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ValidateMintWithFreezeInterfaceCheckInstruction {
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
    }
    .into();

    // On-chain mint has no freeze_authority, but handler expects one
    let result = svm.process_instruction(
        &instruction,
        &[
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "should fail: on-chain mint has no freeze authority"
    );
}
