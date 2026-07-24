use {
    crate::compat::{Instruction, Pubkey},
    crate::helpers::*,
    quasar_test_token_cpi::cpi::*,
};

// Approve discriminator 1 with Program<Token>.

#[test]
fn approve_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ApproveInstruction {
        authority,
        source: source_key,
        delegate: delegate_key,
        amount: 500,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            signer_account(delegate_key),
        ],
    );
    assert!(
        result.is_ok(),
        "approve SPL should succeed: {:?}",
        result.raw_result
    );
}

// ApproveT22 discriminator 22 with Program<Token2022>.

#[test]
fn approve_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = ApproveT22Instruction {
        authority,
        source: source_key,
        delegate: delegate_key,
        amount: 500,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            signer_account(delegate_key),
        ],
    );
    assert!(
        result.is_ok(),
        "approve T22 should succeed: {:?}",
        result.raw_result
    );
}

// ApproveInterface discriminator 23 with Interface<TokenInterface>.

#[test]
fn approve_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ApproveInterfaceInstruction {
        authority,
        source: source_key,
        delegate: delegate_key,
        token_program,
        amount: 500,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account(source_key, mint_key, authority, 1000, token_program),
            signer_account(delegate_key),
        ],
    );
    assert!(
        result.is_ok(),
        "approve interface SPL should succeed: {:?}",
        result.raw_result
    );
}

// Revoke discriminator 2 with Program<Token>.

#[test]
fn revoke_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = RevokeInstruction {
        authority,
        source: source_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account_with_delegate(
                source_key,
                mint_key,
                authority,
                1000,
                delegate_key,
                500,
                token_program,
            ),
        ],
    );
    assert!(
        result.is_ok(),
        "revoke SPL should succeed: {:?}",
        result.raw_result
    );
}

// RevokeT22 discriminator 24 with Program<Token2022>.

// RevokeInterface discriminator 25 with Interface<TokenInterface>.

#[test]
fn revoke_interface_spl() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = RevokeInterfaceInstruction {
        authority,
        source: source_key,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account_with_delegate(
                source_key,
                mint_key,
                authority,
                1000,
                delegate_key,
                500,
                token_program,
            ),
        ],
    );
    assert!(
        result.is_ok(),
        "revoke interface SPL should succeed: {:?}",
        result.raw_result
    );
}

// Error propagation through the CPI machinery: the SPL program's own
// rejection must surface exactly, not be masked or remapped.

#[test]
fn approve_rejects_wrong_owner() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_owner = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = ApproveInstruction {
        authority,
        source: source_key,
        delegate: delegate_key,
        amount: 500,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            // The source's internal owner is someone else: the framework's
            // bare Account<Token> only checks the program owner, so SPL's
            // validate_owner must be the check that fires.
            token_account(source_key, mint_key, wrong_owner, 1000, token_program),
            signer_account(delegate_key),
        ],
    );
    // spl_token::TokenError::OwnerMismatch = 4
    result.assert_error(crate::compat::ProgramError::Custom(4));
}

#[test]
fn revoke_rejects_wrong_owner() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let wrong_owner = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let instruction: Instruction = RevokeInstruction {
        authority,
        source: source_key,
    }
    .into();
    let result = svm.process_instruction(
        &instruction,
        &[
            signer_account(authority),
            token_account_with_delegate(
                source_key,
                mint_key,
                wrong_owner,
                1000,
                delegate_key,
                500,
                token_program,
            ),
        ],
    );
    // spl_token::TokenError::OwnerMismatch = 4
    result.assert_error(crate::compat::ProgramError::Custom(4));
}
