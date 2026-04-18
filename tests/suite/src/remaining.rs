use {
    crate::helpers::mollusk_for_program,
    mollusk_svm::{result::ProgramResult, Mollusk},
    quasar_lang::{error::QuasarError, prelude::ProgramError},
    quasar_test_misc::cpi::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_misc::ID, "quasar_test_misc")
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

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![
            AccountMeta::new_readonly(extra1, false),
            AccountMeta::new_readonly(extra2, false),
        ],
    }
    .into();

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

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![],
    }
    .into();

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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..=64 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![AccountMeta::new_readonly(extra, false)],
    }
    .into();

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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..5 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..10 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..3 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta {
            pubkey: addr,
            is_signer: true,
            is_writable: false,
        });
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![
            AccountMeta {
                pubkey: signer_addr,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta::new(writable_addr, false),
            AccountMeta::new_readonly(readonly_addr, false),
        ],
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (signer_addr, Account::new(1_000_000, 0, &Address::default())),
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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..64 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let mut remaining = Vec::new();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..65 {
        let addr = Address::new_unique();
        remaining.push(AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into();

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

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![AccountMeta::new_readonly(system_program, false)],
    }
    .into();

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
// Remaining Accounts: duplicate handling
// ============================================================================

#[test]
fn test_remaining_duplicate_of_declared_rejected() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![AccountMeta::new_readonly(authority, false)],
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account.clone()),
            (authority, authority_account),
        ],
    );

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(
            QuasarError::RemainingAccountDuplicate as u32
        )),
        "strict remaining accounts should reject duplicates of declared accounts"
    );
}

#[test]
fn test_remaining_duplicate_of_prior_remaining_rejected() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let extra = Address::new_unique();

    let instruction: Instruction = RemainingAccountsCheckInstruction {
        authority,
        remaining_accounts: vec![
            AccountMeta::new_readonly(extra, false),
            AccountMeta::new_readonly(extra, false),
        ],
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (extra, Account::new(1_000_000, 0, &Address::default())),
            (extra, Account::new(1_000_000, 0, &Address::default())),
        ],
    );

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(
            QuasarError::RemainingAccountDuplicate as u32
        )),
        "strict remaining accounts should reject duplicates of prior remaining accounts"
    );
}

#[test]
fn test_remaining_passthrough_duplicate_of_declared() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = RemainingAccountsPassthroughCheckInstruction {
        authority,
        remaining_accounts: vec![AccountMeta::new_readonly(authority, false)],
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account.clone()),
            (authority, authority_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "passthrough remaining accounts should preserve duplicates of declared accounts: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_passthrough_duplicate_of_prior_remaining() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let extra = Address::new_unique();

    let instruction: Instruction = RemainingAccountsPassthroughCheckInstruction {
        authority,
        remaining_accounts: vec![
            AccountMeta::new_readonly(extra, false),
            AccountMeta::new_readonly(extra, false),
        ],
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (extra, Account::new(1_000_000, 0, &Address::default())),
            (extra, Account::new(1_000_000, 0, &Address::default())),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "passthrough remaining accounts should preserve duplicates of prior remaining accounts: \
         {:?}",
        result.program_result
    );
}
