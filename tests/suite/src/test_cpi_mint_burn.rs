use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_cpi::cpi::*,
};

// MintTo discriminator 3 with Program<Token>.

#[test]
fn mint_to_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = MintToInstruction {
        authority,
        mint: mint_key,
        to: to_key,
        amount: 5000,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "mint_to SPL should succeed: {:?}",
        result.raw_result
    );
}

// MintToT22 discriminator 26 with Program<Token2022>.

#[test]
fn mint_to_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = MintToT22Instruction {
        authority,
        mint: mint_key,
        to: to_key,
        amount: 5000,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "mint_to T22 should succeed: {:?}",
        result.raw_result
    );
}

// MintToInterface discriminator 27 with Interface<TokenInterface>.

#[test]
fn mint_to_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = MintToInterfaceInstruction {
        authority,
        mint: mint_key,
        to: to_key,
        token_program,
        amount: 5000,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "mint_to interface SPL should succeed: {:?}",
        result.raw_result
    );
}

// Burn discriminator 4 with Program<Token>.

#[test]
fn burn_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = BurnInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        amount: 500,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 1000, token_program),
            mint_account(mint_key, authority, 9, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "burn SPL should succeed: {:?}",
        result.raw_result
    );
}

// BurnT22 discriminator 28 with Program<Token2022>.

// BurnInterface discriminator 29 with Interface<TokenInterface>.

#[test]
fn burn_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = BurnInterfaceInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        token_program,
        amount: 500,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 1000, token_program),
            mint_account(mint_key, authority, 9, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "burn interface SPL should succeed: {:?}",
        result.raw_result
    );
}

// Error propagation through the CPI machinery: the SPL program's own
// rejection must surface exactly, not be masked or remapped.

#[test]
fn mint_to_rejects_wrong_mint_authority() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = MintToInstruction {
        authority,
        mint: mint_key,
        to: to_key,
        amount: 5000,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            // The mint's authority is someone else: SPL validate_owner fails.
            mint_account(mint_key, wrong_authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    // spl_token::TokenError::OwnerMismatch = 4
    result.assert_error(quasar_svm::ProgramError::Custom(4));
}

#[test]
fn burn_rejects_more_than_balance() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = BurnInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        amount: 500,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 9, token_program),
        ],
    );
    // spl_token::TokenError::InsufficientFunds = 1
    result.assert_error(quasar_svm::ProgramError::Custom(1));
}
