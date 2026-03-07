use mollusk_svm::result::ProgramResult as MolluskResult;
use mollusk_svm::Mollusk;
use quasar_core::prelude::ProgramError;
use quasar_test_errors::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

const ERROR_ACCOUNT_SIZE: usize = 41;

const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0u8; 32]);

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_errors::ID,
        "../../target/deploy/quasar_test_errors",
    )
}

fn build_valid_account_data(authority: Address, value: u64) -> Vec<u8> {
    let mut data = vec![0u8; ERROR_ACCOUNT_SIZE];
    data[0] = 1;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data
}

// ============================================================================
// Account<T> — Owner Validation
// ============================================================================

#[test]
fn test_account_wrong_owner() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: wrong_owner,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::IllegalOwner)
    );
}

// ============================================================================
// Account<T> — Discriminator Validation
// ============================================================================

#[test]
fn test_account_wrong_discriminator() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let mut data = vec![0u8; ERROR_ACCOUNT_SIZE];
    data[0] = 99;
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data,
                owner: quasar_test_errors::ID,
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
// Account<T> — Data Size Validation
// ============================================================================

#[test]
fn test_account_data_too_small() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: vec![1u8; 5],
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::AccountDataTooSmall)
    );
}

#[test]
fn test_account_data_exactly_minimum_size() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 0);
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_account_data_one_byte_short() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let data = vec![1u8; ERROR_ACCOUNT_SIZE - 1];
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::AccountDataTooSmall)
    );
}

// ============================================================================
// Account<T> — Uninitialized / All-Zero Discriminator
// ============================================================================

#[test]
fn test_account_all_zero_discriminator() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let data = vec![0u8; ERROR_ACCOUNT_SIZE];
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data,
                owner: quasar_test_errors::ID,
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

#[test]
fn test_account_not_initialized_empty_data() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::AccountDataTooSmall)
    );
}

// ============================================================================
// Account<T> — Duplicate Account Detection
// ============================================================================

#[test]
fn test_account_duplicate_same_account_two_params() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = TwoAccountsCheckInstruction {
        first: account_addr,
        second: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data.clone(),
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::AccountBorrowFailed)
    );
}

// ============================================================================
// Account<T> — Happy Path
// ============================================================================

#[test]
fn test_account_success_valid_owner_disc_data() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_account_success_with_extra_data() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let mut account_data = build_valid_account_data(Address::new_unique(), 100);
    account_data.extend_from_slice(&[0u8; 64]);
    let instruction: Instruction = AccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

// ============================================================================
// Account<T> with #[account(mut)] — Mutability Validation
// ============================================================================

#[test]
fn test_mut_account_success() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = MutAccountCheckInstruction {
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_mut_account_not_writable() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(account_addr, false)],
        data: vec![10],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Immutable)
    );
}

// ============================================================================
// Signer — Validation
// ============================================================================

#[test]
fn test_signer_success() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = SignerReadonlyCheckInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_signer_not_signer() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(signer, false)],
        data: vec![25],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::MissingRequiredSignature)
    );
}

#[test]
fn test_signer_mut_success() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = SignerMutCheckInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_signer_mut_not_signer() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new(signer, false)],
        data: vec![22],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::MissingRequiredSignature)
    );
}

#[test]
fn test_signer_mut_not_writable() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(signer, true)],
        data: vec![22],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Immutable)
    );
}

// ============================================================================
// SystemAccount — Validation
// ============================================================================

#[test]
fn test_system_account_success() {
    let mollusk = setup();
    let account = Address::new_unique();
    let instruction: Instruction = SystemAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_system_account_wrong_owner() {
    let mollusk = setup();
    let account = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let instruction: Instruction = SystemAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: wrong_owner,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::IllegalOwner)
    );
}

#[test]
fn test_system_account_owned_by_program() {
    let mollusk = setup();
    let account = Address::new_unique();
    let instruction: Instruction = SystemAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::IllegalOwner)
    );
}

// ============================================================================
// Program<T> — Validation
// ============================================================================

#[test]
fn test_program_success() {
    let mollusk = setup();
    let instruction: Instruction = ProgramCheckInstruction {
        program: SYSTEM_PROGRAM_ID,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            SYSTEM_PROGRAM_ID,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: true,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_program_wrong_id() {
    let mollusk = setup();
    let wrong_id = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(wrong_id, false)],
        data: vec![21],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            wrong_id,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: SYSTEM_PROGRAM_ID,
                executable: true,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::IncorrectProgramId)
    );
}

#[test]
fn test_program_not_executable() {
    let mollusk = setup();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false)],
        data: vec![21],
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
// UncheckedAccount — Validation (minimal, accepts anything)
// ============================================================================

#[test]
fn test_unchecked_account_success_empty() {
    let mollusk = setup();
    let account = Address::new_unique();
    let instruction: Instruction = UncheckedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(account, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_unchecked_account_success_any_owner() {
    let mollusk = setup();
    let account = Address::new_unique();
    let random_owner = Address::new_unique();
    let instruction: Instruction = UncheckedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account,
            Account {
                lamports: 1_000_000,
                data: vec![1, 2, 3, 4],
                owner: random_owner,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_unchecked_account_success_with_data() {
    let mollusk = setup();
    let account = Address::new_unique();
    let instruction: Instruction = UncheckedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account,
            Account {
                lamports: 500_000,
                data: vec![0u8; 256],
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

// ============================================================================
// Two Account<T> Fields — Distinct Accounts (Happy Path)
// ============================================================================

#[test]
fn test_two_accounts_distinct_success() {
    let mollusk = setup();
    let first_addr = Address::new_unique();
    let second_addr = Address::new_unique();
    let first_data = build_valid_account_data(Address::new_unique(), 1);
    let second_data = build_valid_account_data(Address::new_unique(), 2);
    let instruction: Instruction = TwoAccountsCheckInstruction {
        first: first_addr,
        second: second_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                first_addr,
                Account {
                    lamports: 1_000_000,
                    data: first_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                second_addr,
                Account {
                    lamports: 1_000_000,
                    data: second_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_two_accounts_first_wrong_owner() {
    let mollusk = setup();
    let first_addr = Address::new_unique();
    let second_addr = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let first_data = build_valid_account_data(Address::new_unique(), 1);
    let second_data = build_valid_account_data(Address::new_unique(), 2);
    let instruction: Instruction = TwoAccountsCheckInstruction {
        first: first_addr,
        second: second_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                first_addr,
                Account {
                    lamports: 1_000_000,
                    data: first_data,
                    owner: wrong_owner,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                second_addr,
                Account {
                    lamports: 1_000_000,
                    data: second_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::IllegalOwner)
    );
}

#[test]
fn test_two_accounts_second_wrong_discriminator() {
    let mollusk = setup();
    let first_addr = Address::new_unique();
    let second_addr = Address::new_unique();
    let first_data = build_valid_account_data(Address::new_unique(), 1);
    let mut second_data = vec![0u8; ERROR_ACCOUNT_SIZE];
    second_data[0] = 99;
    let instruction: Instruction = TwoAccountsCheckInstruction {
        first: first_addr,
        second: second_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                first_addr,
                Account {
                    lamports: 1_000_000,
                    data: first_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (
                second_addr,
                Account {
                    lamports: 1_000_000,
                    data: second_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::InvalidAccountData)
    );
}

// ============================================================================
// has_one (Default Error) — HasOneMismatch (3005)
// ============================================================================

#[test]
fn test_has_one_default_success() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(authority, 42);
    let instruction: Instruction = HasOneDefaultInstruction {
        authority,
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_has_one_default_mismatch() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(wrong_authority, 42);
    let instruction: Instruction = HasOneDefaultInstruction {
        authority,
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(3005))
    );
}

// ============================================================================
// address (Default Error) — AddressMismatch (3012)
// ============================================================================

#[test]
fn test_address_default_success() {
    let mollusk = setup();
    let expected_addr = Address::new_from_array([88u8; 32]);
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AddressDefaultInstruction {
        target: expected_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            expected_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_address_default_mismatch() {
    let mollusk = setup();
    let wrong_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AddressDefaultInstruction { target: wrong_addr }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            wrong_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(3012))
    );
}

// ============================================================================
// constraint (Default Error) — ConstraintViolation (3004)
// ============================================================================

#[test]
fn test_constraint_default_fails() {
    let mollusk = setup();
    let target = Address::new_unique();
    let instruction: Instruction = ConstraintDefaultInstruction { target }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            target,
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
        MolluskResult::Failure(ProgramError::Custom(3004))
    );
}

// ============================================================================
// has_one (Custom Error) — TestError::Hello (0)
// ============================================================================

#[test]
fn test_has_one_custom_success() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(authority, 42);
    let instruction: Instruction = HasOneCustomInstruction {
        authority,
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_has_one_custom_mismatch() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_valid_account_data(wrong_authority, 42);
    let instruction: Instruction = HasOneCustomInstruction {
        authority,
        account: account_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_errors::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(0))
    );
}

// ============================================================================
// address (Custom Error) — TestError::AddressCustom (104)
// ============================================================================

#[test]
fn test_address_custom_success() {
    let mollusk = setup();
    let expected_addr = Address::new_from_array([99u8; 32]);
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AddressCustomErrorInstruction {
        target: expected_addr,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            expected_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_address_custom_mismatch() {
    let mollusk = setup();
    let wrong_addr = Address::new_unique();
    let account_data = build_valid_account_data(Address::new_unique(), 42);
    let instruction: Instruction = AddressCustomErrorInstruction { target: wrong_addr }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            wrong_addr,
            Account {
                lamports: 1_000_000,
                data: account_data,
                owner: quasar_test_errors::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(104))
    );
}

// ============================================================================
// constraint (Custom Error) — TestError::ConstraintCustom (103)
// ============================================================================

#[test]
fn test_constraint_custom_fails() {
    let mollusk = setup();
    let target = Address::new_unique();
    let instruction: Instruction = ConstraintFailInstruction { target }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(target, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(103))
    );
}
