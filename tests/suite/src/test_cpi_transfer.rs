use {
    crate::compat::{Instruction, Pubkey},
    crate::helpers::*,
    quasar_test_token_cpi::cpi::*,
};

// TransferChecked discriminator 0 with Program<Token>.

/// Compute ceiling for SPL `transfer_checked` through the generated CPI builder.
///
/// These programs carry the derive's per-instruction account walk across many
/// instructions, so they are what an inlining or parse-shape change moves
/// first. Without a ceiling here such a change is only ever measured by
/// binary size. Re-measure deliberately rather than raising this.
const MAX_TRANSFER_CHECKED_CU: u64 = 7_600;

#[test]
fn transfer_checked_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = TransferCheckedInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        to: to_key,
        amount: 200,
        decimals: 9,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 500, token_program),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "transfer_checked SPL should succeed: {:?}",
        result.raw_result
    );
    assert!(
        result.compute_units_consumed <= MAX_TRANSFER_CHECKED_CU,
        "transfer_checked_spl should stay within {} CU, consumed {}",
        MAX_TRANSFER_CHECKED_CU,
        result.compute_units_consumed
    );
}

// TransferCheckedT22 discriminator 20 with Program<Token2022>.

#[test]
fn transfer_checked_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = TransferCheckedT22Instruction {
        authority,
        from: from_key,
        mint: mint_key,
        to: to_key,
        amount: 200,
        decimals: 9,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 500, token_program),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "transfer_checked T22 should succeed: {:?}",
        result.raw_result
    );
}

// TransferCheckedInterface discriminator 21 with Interface<TokenInterface>.

#[test]
fn transfer_checked_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = TransferCheckedInterfaceInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        to: to_key,
        token_program,
        amount: 200,
        decimals: 9,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 500, token_program),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "transfer_checked interface SPL should succeed: {:?}",
        result.raw_result
    );
}

// InterfaceTransfer discriminator 6, unchecked transfer via Interface.

#[test]
fn interface_transfer_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = InterfaceTransferInstruction {
        authority,
        from: from_key,
        to: to_key,
        token_program,
        amount: 300,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 500, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "interface_transfer SPL should succeed: {:?}",
        result.raw_result
    );
}

// Error propagation through the CPI machinery: the SPL program's own
// rejection must surface exactly, not be masked or remapped.

#[test]
fn transfer_checked_rejects_insufficient_funds() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = TransferCheckedInstruction {
        authority,
        from: from_key,
        mint: mint_key,
        to: to_key,
        amount: 200,
        decimals: 9,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 100, token_program),
            mint_account(mint_key, authority, 9, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    // spl_token::TokenError::InsufficientFunds = 1
    result.assert_error(crate::compat::ProgramError::Custom(1));
}

#[test]
fn interface_transfer_rejects_insufficient_funds() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let from_key = Pubkey::new_unique();
    let to_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = InterfaceTransferInstruction {
        authority,
        from: from_key,
        to: to_key,
        token_program,
        amount: 300,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(from_key, mint_key, authority, 100, token_program),
            token_account(to_key, mint_key, authority, 0, token_program),
        ],
    );
    // spl_token::TokenError::InsufficientFunds = 1
    result.assert_error(crate::compat::ProgramError::Custom(1));
}
