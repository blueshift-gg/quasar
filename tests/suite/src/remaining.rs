use mollusk_svm::result::ProgramResult;
use mollusk_svm::Mollusk;
use quasar_core::error::QuasarError;
use quasar_core::prelude::ProgramError;
use quasar_test_misc::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_misc::ID,
        "../../target/deploy/quasar_test_misc",
    )
}

// ============================================================================
// Remaining Accounts (discriminator 16)
// ============================================================================

#[test]
fn test_remaining_accounts_with_extras() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let extra1 = Address::new_unique();
    let extra2 = Address::new_unique();

    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let extra1_account = Account::new(1_000_000, 0, &Address::default());
    let extra2_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(extra1, false));
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(extra2, false));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (extra1, extra1_account),
            (extra2, extra2_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with extras should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_accounts_empty() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();

    let result = mollusk.process_instruction(&instruction, &[(authority, authority_account)]);

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with no extras should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_accounts_overflow_errors() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..=64 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(
            QuasarError::RemainingAccountsOverflow as u32
        ))
    );
}

// ============================================================================
// Remaining Accounts: one account
// ============================================================================

#[test]
fn test_remaining_one_account() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let extra = Address::new_unique();

    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let extra_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(extra, false));

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (extra, extra_account)],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining with 1 account should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: five accounts
// ============================================================================

#[test]
fn test_remaining_five_accounts() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..5 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert!(
        result.program_result.is_ok(),
        "remaining with 5 accounts should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: ten accounts
// ============================================================================

#[test]
fn test_remaining_ten_accounts() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..10 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert!(
        result.program_result.is_ok(),
        "remaining with 10 accounts should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: all signers
// ============================================================================

#[test]
fn test_remaining_accounts_all_signers() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..3 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta {
                pubkey: addr,
                is_signer: true,
                is_writable: false,
            });
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert!(
        result.program_result.is_ok(),
        "remaining accounts all signers should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: mixed flags
// ============================================================================

#[test]
fn test_remaining_accounts_mixed_flags() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let signer_addr = Address::new_unique();
    let writable_addr = Address::new_unique();
    let readonly_addr = Address::new_unique();

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta {
            pubkey: signer_addr,
            is_signer: true,
            is_writable: false,
        });
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new(writable_addr, false));
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(
            readonly_addr,
            false,
        ));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (
                signer_addr,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                writable_addr,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                readonly_addr,
                Account::new(1_000_000, 0, &Address::default()),
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with mixed flags should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: exactly 64 (max)
// ============================================================================

#[test]
fn test_remaining_64_accounts_max() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..64 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert!(
        result.program_result.is_ok(),
        "remaining with exactly 64 accounts should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: 65 overflows
// ============================================================================

#[test]
fn test_remaining_65_accounts_overflow() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..65 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(
            QuasarError::RemainingAccountsOverflow as u32
        )),
        "remaining with 65 accounts should overflow"
    );
}

// ============================================================================
// Remaining Accounts: include system program
// ============================================================================

#[test]
fn test_remaining_accounts_include_system_program() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let system_program = Address::default();

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(
            system_program,
            false,
        ));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (
                system_program,
                Account::new(1, 0, &Address::new_from_array([1u8; 32])),
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with system program should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts: duplicate of declared account
// ============================================================================

#[test]
fn test_remaining_duplicate_of_declared() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(
            authority, false,
        ));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account.clone()),
            (authority, authority_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with duplicate of declared account should succeed: {:?}",
        result.program_result
    );
}
