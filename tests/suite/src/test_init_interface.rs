use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::cpi::*,
};

// ===========================================================================
// init InterfaceAccount<Token> — with SPL Token program
// ===========================================================================

#[test]
fn init_token_interface_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenInterfaceInstruction {
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
        "init InterfaceAccount<Token> with SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_token_interface_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitTokenInterfaceInstruction {
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
        "init InterfaceAccount<Token> with T22 should succeed: {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed InterfaceAccount<Token> — new + existing + adversarial
// ===========================================================================

#[test]
fn init_if_needed_token_interface_spl_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededTokenInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Token> new (SPL) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_token_interface_t22_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededTokenInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Token> new (T22) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_token_interface_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Token> existing valid should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_token_interface_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let token_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededTokenInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Token> wrong mint should fail"
    );
}

// ===========================================================================
// init InterfaceAccount<Mint>
// ===========================================================================

#[test]
fn init_mint_interface_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintInterfaceInstruction {
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
        "init InterfaceAccount<Mint> with SPL should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_mint_interface_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitMintInterfaceInstruction {
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
        "init InterfaceAccount<Mint> with T22 should succeed: {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed InterfaceAccount<Mint> — new + existing + adversarial
// ===========================================================================

#[test]
fn init_if_needed_mint_interface_spl_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Mint> new (SPL) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_interface_t22_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction = with_signers(
        InitIfNeededMintInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Mint> new (T22) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_interface_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Mint> existing valid should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_if_needed_mint_interface_existing_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let system_program = quasar_svm::system_program::ID;

    let instruction: Instruction = InitIfNeededMintInterfaceInstruction {
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
        "init_if_needed InterfaceAccount<Mint> wrong authority should fail"
    );
}
