use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::cpi::*,
};

// init with SPL Token.

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
    result.assert_error(quasar_svm::ProgramError::AccountAlreadyInitialized);
}

// init with Token-2022.

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
    result.assert_error(quasar_svm::ProgramError::AccountAlreadyInitialized);
}

// init_if_needed new account with SPL Token.

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

// init_if_needed existing valid account with SPL Token.

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

    let existing = token_account(token_key, mint_key, payer, 100, token_program);
    let existing_data = existing.data.clone();
    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            existing,
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid token should succeed (no-op): {:?}",
        result.raw_result
    );
    // "No-op" must mean untouched: the existing account's bytes are
    // byte-identical after the idempotent init.
    let after = result.account(&token_key).expect("existing account");
    assert_eq!(
        after.data, existing_data,
        "existing valid account must be left unmodified"
    );
}

// init_if_needed existing invalid account with SPL Token.

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
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
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
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
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
    // The existing account is system-owned, so init takes the create
    // branch: SystemError::AccountAlreadyInUse.
    result.assert_error(quasar_svm::ProgramError::Custom(0));
}

// init_if_needed new account with Token-2022.

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

// init_if_needed existing valid account with Token-2022.

// init_if_needed existing invalid account with Token-2022.
