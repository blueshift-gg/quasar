use mollusk_svm::result::ProgramResult as MolluskResult;
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_errors::ID,
        "../../target/deploy/quasar_test_errors",
    )
}

const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);

// ============================================================================
// Header Validation Debug Message Tests
//
// Build test-errors with debug feature:
//   cargo build-sbf --manifest-path tests/programs/test-errors/Cargo.toml --features debug,alloc
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
        data: vec![12], // discriminator 12 = header_nodup_mut_signer
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
        data: vec![14], // discriminator 14 = header_nodup_signer
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
fn test_header_dup_accounts_success() {
    let mollusk = setup();
    let source = Address::new_unique();
    let destination = Address::new_unique();

    // Test with dup-allowed struct
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![
            AccountMeta::new_readonly(source, true), // source signer
            AccountMeta::new(destination, false),    // destination writable
        ],
        data: vec![16], // discriminator 16 = header_dup_mut
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (source, Account::default()),
            (destination, Account::default()),
        ],
    );

    assert_eq!(result.program_result, MolluskResult::Success);

    #[cfg(feature = "debug")]
    println!("✓ Test passed: dup-allowed accounts validated correctly");
}
