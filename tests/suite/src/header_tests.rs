use {
    crate::helpers::mollusk_for_program,
    mollusk_svm::{result::ProgramResult as MolluskResult, Mollusk},
    quasar_lang::prelude::ProgramError,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_errors::ID, "quasar_test_errors")
}

const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);

// ============================================================================
// Header Validation Debug Message Tests
//
// Build test-errors with debug feature:
//   cargo build-sbf --manifest-path tests/programs/test-errors/Cargo.toml
// --features debug,alloc
//
// Run with:
//   cargo test -p quasar-test-suite --features debug -- test_header --nocapture
//
// The tests verify that:
// 1. Account header validation works correctly
// 2. Debug messages are emitted with the correct account name and constraint
// ============================================================================

#[test]
fn test_header_nodup_mut_signer_success() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Correctly pass a writable signer
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new(account, true)], // writable + signer
        data: vec![12],                                  /* discriminator 12 =
                                                          * header_nodup_mut_signer */
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(result.program_result, MolluskResult::Success);

    #[cfg(feature = "debug")]
    println!("✓ Test passed: writable signer account validated correctly");
}

#[test]
fn test_header_nodup_mut_success() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Correctly pass a writable account
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new(account, false)], // writable only
        data: vec![13],                                   // discriminator 13 = header_nodup_mut
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(result.program_result, MolluskResult::Success);

    #[cfg(feature = "debug")]
    println!("✓ Test passed: writable account validated correctly");
}

#[test]
fn test_header_nodup_signer_success() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Correctly pass a signer
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(account, true)], // signer only
        data: vec![14],                                           /* discriminator 14 =
                                                                   * header_nodup_signer */
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(result.program_result, MolluskResult::Success);

    #[cfg(feature = "debug")]
    println!("✓ Test passed: signer account validated correctly");
}

#[test]
fn test_header_executable_success() {
    let mollusk = setup();

    // System program is executable
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false)],
        data: vec![15], // discriminator 15 = header_executable
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID, // Self-owned
                executable: true,         // System program is executable
                rent_epoch: 0,
            },
        )],
    );

    assert_eq!(result.program_result, MolluskResult::Success);

    #[cfg(feature = "debug")]
    println!("✓ Test passed: executable program validated correctly");
}

#[test]
fn test_header_dup_accounts_distinct_address_accepted() {
    let mollusk = setup();
    let source = Address::new_unique();
    let destination = Address::new_unique();

    // `#[account(dup)]` means "MAY alias" — distinct addresses are fine.
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new_readonly(source, true),
            AccountMeta::new_readonly(destination, false),
        ],
        data: vec![16], // discriminator 16 = header_dup_readonly
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (source, Account::default()),
            (destination, Account::default()),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "dup field with distinct address should succeed: {:?}",
        result.program_result,
    );
}

// ============================================================================
// Failure Cases — Non-signer passed where signer required
// ============================================================================

#[test]
fn test_header_nodup_signer_fails_not_signer() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(account, false)],
        data: vec![14],
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::MissingRequiredSignature)
    );
}

// ============================================================================
// Failure Cases — Non-writable passed where writable required
// ============================================================================

#[test]
fn test_header_nodup_mut_fails_not_writable() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(account, false)],
        data: vec![13],
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Immutable)
    );
}

// ============================================================================
// Failure Cases — Non-writable non-signer passed where both required
// ============================================================================

#[test]
fn test_header_nodup_mut_signer_fails_not_signer() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new(account, false)],
        data: vec![12],
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::MissingRequiredSignature)
    );
}

#[test]
fn test_header_nodup_mut_signer_fails_not_writable() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(account, true)],
        data: vec![12],
    };

    let result = mollusk.process_instruction(&instruction, &[(account, Account::default())]);

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Immutable)
    );
}

// ============================================================================
// Failure Cases — Not executable
// ============================================================================

#[test]
fn test_header_executable_fails_not_executable() {
    let mollusk = setup();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false)],
        data: vec![15],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::InvalidAccountData)
    );
}

// ============================================================================
// Failure Cases — Duplicate account where no-dup required
// ============================================================================

#[test]
fn test_header_three_way_duplicate() {
    // Three slots with the same account: signer, writable, readonly.
    // The second slot (mutable) is a dup of the first (immutable signer).
    // Mutable dups are rejected unless #[account(dup)] is present.
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new_readonly(account, true),
            AccountMeta::new(account, false),
            AccountMeta::new_readonly(account, false),
        ],
        data: vec![26],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, Account::default()),
            (account, Account::default()),
            (account, Account::default()),
        ],
    );

    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::AccountBorrowFailed)
    );
}

// ============================================================================
// Dup-allowed accounts — Duplicate with different flags
// ============================================================================

#[test]
fn test_header_dup_readonly_same_account_success() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new_readonly(account, true),
            AccountMeta::new_readonly(account, false),
        ],
        data: vec![16],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, Account::default()), (account, Account::default())],
    );

    assert_eq!(result.program_result, MolluskResult::Success);
}

#[test]
fn test_header_dup_readonly_writable_alias_still_parses() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new_readonly(account, true),
            AccountMeta::new(account, false),
        ],
        data: vec![16],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, Account::default()), (account, Account::default())],
    );

    assert_eq!(result.program_result, MolluskResult::Success);
}

#[test]
fn test_header_dup_signer_same_account_success() {
    let mollusk = setup();
    let account = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new(account, true),
            AccountMeta::new_readonly(account, true),
        ],
        data: vec![17],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, Account::default()), (account, Account::default())],
    );

    assert_eq!(result.program_result, MolluskResult::Success);
}
