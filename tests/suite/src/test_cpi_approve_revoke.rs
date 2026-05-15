use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
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

#[test]
fn approve_interface_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

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
        "approve interface T22 should succeed: {:?}",
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
        "revoke SPL should succeed: {:?}",
        result.raw_result
    );
}

// RevokeT22 discriminator 24 with Program<Token2022>.

#[test]
fn revoke_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let instruction: Instruction = RevokeT22Instruction {
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
        "revoke T22 should succeed: {:?}",
        result.raw_result
    );
}

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

#[test]
fn revoke_interface_t22() {
    let mut svm = svm_cpi();
    let authority = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_key = Pubkey::new_unique();
    let delegate_key = Pubkey::new_unique();
    let token_program = token_2022_program_id();

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
        "revoke interface T22 should succeed: {:?}",
        result.raw_result
    );
}
