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

// Remaining Accounts, discriminator 16.

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

// Remaining Accounts: one account.

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

// Remaining Accounts: five accounts.

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

// Remaining Accounts: ten accounts.

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

// Remaining Accounts: all signers.

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

// Remaining Accounts: mixed flags.

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

// Remaining Accounts: exactly 64, the max.

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

// Remaining Accounts: 65 overflows.

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

// Remaining Accounts: include system program.

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

// Remaining Accounts: duplicate handling.

#[test]
fn test_remaining_duplicate_of_declared_allowed() {
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

    assert!(
        result.program_result.is_ok(),
        "remaining accounts should preserve duplicates of declared accounts: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_duplicate_of_prior_remaining_allowed() {
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

    assert!(
        result.program_result.is_ok(),
        "remaining accounts should preserve duplicates of prior remaining accounts: {:?}",
        result.program_result
    );
}

// Typed remaining-accounts parsing (test-errors disc 30): unlike the raw
// iterator above, parse::<Account<T>, N>() runs duplicate rejection and the
// full owner/discriminator/length load on every remaining entry. The raw
// walk's memory safety is proven by the lang/kani/remaining.rs harnesses;
// these tests pin the *semantic* rejections, which Kani does not model.

use {
    crate::helpers::{raw_account, signer_account, svm_errors},
    quasar_test_errors::cpi as err_cpi,
};

/// ErrorTestAccount: disc=1 (1) + authority (32) + value u64 (8) = 41 bytes.
const ERROR_TEST_ACCOUNT_SIZE: usize = 41;

fn error_test_account(
    address: crate::compat::Pubkey,
    owner: crate::compat::Pubkey,
) -> crate::compat::Account {
    let mut data = vec![0u8; ERROR_TEST_ACCOUNT_SIZE];
    data[0] = 1;
    raw_account(address, 1_000_000, data, owner)
}

fn remaining_typed_ix(
    authority: crate::compat::Pubkey,
    remaining: Vec<AccountMeta>,
) -> crate::compat::Instruction {
    err_cpi::RemainingTypedCheckInstruction {
        authority,
        remaining_accounts: remaining,
    }
    .into()
}

#[test]
fn typed_remaining_parses_valid_accounts() {
    let mut svm = svm_errors();
    let authority = crate::compat::Pubkey::new_unique();
    let a = crate::compat::Pubkey::new_unique();
    let b = crate::compat::Pubkey::new_unique();

    let ix = remaining_typed_ix(
        authority,
        vec![
            AccountMeta::new_readonly(a, false),
            AccountMeta::new_readonly(b, false),
        ],
    );
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(a, quasar_test_errors::ID),
            error_test_account(b, quasar_test_errors::ID),
        ],
    );
    assert!(result.is_ok(), "typed parse: {:?}", result.raw_result);
}

#[test]
fn typed_remaining_rejects_duplicate() {
    let mut svm = svm_errors();
    let authority = crate::compat::Pubkey::new_unique();
    let dup = crate::compat::Pubkey::new_unique();

    let ix = remaining_typed_ix(
        authority,
        vec![
            AccountMeta::new_readonly(dup, false),
            AccountMeta::new_readonly(dup, false),
        ],
    );
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(dup, quasar_test_errors::ID),
        ],
    );
    result.assert_error(crate::compat::ProgramError::Custom(
        QuasarError::RemainingAccountDuplicate as u32,
    ));
}

#[test]
fn typed_remaining_rejects_wrong_owner() {
    let mut svm = svm_errors();
    let authority = crate::compat::Pubkey::new_unique();
    let foreign = crate::compat::Pubkey::new_unique();

    let ix = remaining_typed_ix(authority, vec![AccountMeta::new_readonly(foreign, false)]);
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(foreign, crate::compat::Pubkey::new_unique()),
        ],
    );
    // The harness maps InstructionErrors without a dedicated variant to
    // their Debug string; IllegalOwner is one of those.
    result.assert_error(crate::compat::ProgramError::Runtime("IllegalOwner".into()));
}

#[test]
fn typed_remaining_rejects_truncated_data() {
    let mut svm = svm_errors();
    let authority = crate::compat::Pubkey::new_unique();
    let short = crate::compat::Pubkey::new_unique();

    let mut data = vec![0u8; ERROR_TEST_ACCOUNT_SIZE - 1];
    data[0] = 1; // valid discriminator, one byte short of the layout
    let ix = remaining_typed_ix(authority, vec![AccountMeta::new_readonly(short, false)]);
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            raw_account(short, 1_000_000, data, quasar_test_errors::ID),
        ],
    );
    result.assert_error(crate::compat::ProgramError::AccountDataTooSmall);
}
