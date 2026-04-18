use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::cpi::*,
};

// ===========================================================================
// init — SPL Token
// ===========================================================================

#[test]
fn init_token_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(token_key),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init token should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_token_spl_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, payer, 0, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init on already-initialized account should fail"
    );
}

// ===========================================================================
// init — Token-2022
// ===========================================================================

#[test]
fn init_token_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(token_key),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init token (T22) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_token_t22_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, payer, 0, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init on already-initialized account should fail (T22)"
    );
}

// ===========================================================================
// init_if_needed new — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_token_spl_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededTokenInstruction {
            payer,
            token_account: token_key,
            mint: mint_key,
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
            empty_account(token_key),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on new account should succeed: {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing valid — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_token_spl_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, payer, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid token should succeed (no-op): {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing bad — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_token_spl_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, wrong_mint, payer, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong mint should fail"
    );
}

#[test]
fn init_if_needed_token_spl_existing_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong authority should fail"
    );
}

#[test]
fn init_if_needed_token_spl_existing_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, payer, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong account owner should fail"
    );
}

// ===========================================================================
// init_if_needed new — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_token_t22_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededTokenT22Instruction {
            payer,
            token_account: token_key,
            mint: mint_key,
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
            empty_account(token_key),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on new account should succeed (T22): {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing valid — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_token_t22_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, payer, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid token should succeed (T22, no-op): {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing bad — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_token_t22_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, wrong_mint, payer, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong mint should fail (T22)"
    );
}

#[test]
fn init_if_needed_token_t22_existing_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(token_key, mint_key, wrong_authority, 100, token_program),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong authority should fail (T22)"
    );
}

#[test]
fn init_if_needed_token_t22_existing_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenT22Instruction {
        payer,
        token_account: token_key,
        mint: mint_key,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            raw_account(
                token_key,
                1_000_000,
                pack_token_data(mint_key, payer, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed with wrong account owner should fail (T22)"
    );
}
