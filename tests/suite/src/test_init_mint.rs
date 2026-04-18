use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::cpi::*,
};

// ===========================================================================
// init mint — SPL Token
// ===========================================================================

#[test]
fn init_mint_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init mint should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_mint_spl_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init mint on already-initialized account should fail"
    );
}

// ===========================================================================
// init mint — Token-2022
// ===========================================================================

#[test]
fn init_mint_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init mint (T22) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_mint_t22_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init mint on already-initialized account should fail (T22)"
    );
}

// ===========================================================================
// init_if_needed mint (no freeze) — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_mint_spl_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintInstruction {
            payer,
            mint: mint_key,
            mint_authority: authority,
            token_program,
            system_program,
        }
        .into(),
        &[1],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint on new account should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_spl_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint on existing valid mint should succeed (no-op): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_spl_wrong_decimals() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong decimals should fail"
    );
}

#[test]
fn init_if_needed_mint_spl_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong authority should fail"
    );
}

#[test]
fn init_if_needed_mint_spl_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
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
        "init_if_needed mint with wrong account owner should fail"
    );
}

#[test]
fn init_if_needed_mint_spl_unexpected_freeze() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    // Use the no-freeze handler but provide a mint that has freeze_authority set.
    // validate_mint with freeze_authority=None rejects mints that have one
    // on-chain.
    let instruction: Instruction = InitIfNeededMintInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with unexpected freeze_authority should fail"
    );
}

// ===========================================================================
// init_if_needed mint (no freeze) — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_mint_t22_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintT22Instruction {
            payer,
            mint: mint_key,
            mint_authority: authority,
            token_program,
            system_program,
        }
        .into(),
        &[1],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint on new account should succeed (T22): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_t22_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint on existing valid mint should succeed (T22, no-op): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_t22_wrong_decimals() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 9, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong decimals should fail (T22)"
    );
}

#[test]
fn init_if_needed_mint_t22_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, wrong_authority, 6, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong authority should fail (T22)"
    );
}

#[test]
fn init_if_needed_mint_t22_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
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
        "init_if_needed mint with wrong account owner should fail (T22)"
    );
}

#[test]
fn init_if_needed_mint_t22_unexpected_freeze() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with unexpected freeze_authority should fail (T22)"
    );
}

// ===========================================================================
// init_if_needed mint with freeze — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_mint_freeze_spl_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintWithFreezeInstruction {
            payer,
            mint: mint_key,
            mint_authority: authority,
            freeze_authority: freeze_auth,
            token_program,
            system_program,
        }
        .into(),
        &[1],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint with freeze on new account should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_freeze_spl_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintWithFreezeInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint with freeze on existing valid mint should succeed (no-op): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_freeze_spl_wrong_freeze_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintWithFreezeInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong freeze_authority should fail"
    );
}

#[test]
fn init_if_needed_mint_freeze_spl_missing_freeze_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    // Existing mint has no freeze_authority, but the handler expects one.
    let instruction: Instruction = InitIfNeededMintWithFreezeInstruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with missing freeze_authority on existing mint should fail"
    );
}

// ===========================================================================
// init_if_needed mint with freeze — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_mint_freeze_t22_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintWithFreezeT22Instruction {
            payer,
            mint: mint_key,
            mint_authority: authority,
            freeze_authority: freeze_auth,
            token_program,
            system_program,
        }
        .into(),
        &[1],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(mint_key),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint with freeze on new account should succeed (T22): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_freeze_t22_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintWithFreezeT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, freeze_auth, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed mint with freeze on existing valid mint should succeed (T22, no-op): {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_freeze_t22_wrong_freeze_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let wrong_freeze = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintWithFreezeT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account_with_freeze(mint_key, authority, 6, wrong_freeze, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with wrong freeze_authority should fail (T22)"
    );
}

#[test]
fn init_if_needed_mint_freeze_t22_missing_freeze_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let freeze_auth = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintWithFreezeT22Instruction {
        payer,
        mint: mint_key,
        mint_authority: authority,
        freeze_authority: freeze_auth,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(authority),
            signer_account(freeze_auth),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed mint with missing freeze_authority on existing mint should fail (T22)"
    );
}

// ===========================================================================
// init mint with metadata — SPL Token
// ===========================================================================

// Skipped: InitMintWithMetadata requires the Metaplex Token Metadata program
// to be loaded into the SVM, which QuasarSvm does not bundle. These tests
// would need a custom program loader or mock for the metadata program.
