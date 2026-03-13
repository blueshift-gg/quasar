use {
    mollusk_svm::{result::ProgramResult as MolluskResult, Mollusk},
    quasar_core::prelude::ProgramError,
    quasar_test_errors::client::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
};

const ERROR_ACCOUNT_SIZE: usize = 41;

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_errors::ID,
        "../../target/deploy/quasar_test_errors",
    )
}

fn build_error_test_account_data(authority: Address, value: u64) -> Vec<u8> {
    let mut data = vec![0u8; ERROR_ACCOUNT_SIZE];
    data[0] = 1;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data
}

#[test]
fn test_custom_error_code() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = CustomErrorInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(0))
    );
}

#[test]
fn test_custom_error_with_explicit_number() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = ExplicitErrorInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(100))
    );
}

#[test]
fn test_require_false() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = RequireFalseInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(101))
    );
}

#[test]
fn test_program_error() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = ProgramErrorInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::InvalidAccountData)
    );
}

#[test]
fn test_require_eq_fails() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = RequireEqCheckInstruction { signer, a: 1, b: 2 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(102))
    );
}

#[test]
fn test_require_eq_passes() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = RequireEqCheckInstruction { signer, a: 5, b: 5 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_require_neq_fails() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = RequireNeqCheckInstruction { signer, a: 5, b: 5 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(102))
    );
}

#[test]
fn test_require_neq_passes() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction: Instruction = RequireNeqCheckInstruction { signer, a: 1, b: 2 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
}

#[test]
fn test_constraint_with_custom_error() {
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

#[test]
fn test_has_one_success() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(authority, 42);
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
    assert!(
        result.program_result.is_ok(),
        "has_one should pass when authority matches: {:?}",
        result.program_result
    );
}

#[test]
fn test_has_one_error() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(wrong_authority, 42);
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

#[test]
fn test_signer_error() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(signer, false)],
        data: vec![8],
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
fn test_owner_error() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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

#[test]
fn test_account_check_success() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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
    assert!(
        result.program_result.is_ok(),
        "account_check should pass with valid account: {:?}",
        result.program_result
    );
}

#[test]
fn test_uninitialized_account() {
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
                data: vec![0u8; ERROR_ACCOUNT_SIZE],
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
fn test_data_too_small() {
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
fn test_wrong_discriminator() {
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

#[test]
fn test_mut_account_check_success() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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
    assert!(
        result.program_result.is_ok(),
        "mut_account_check should pass with writable account: {:?}",
        result.program_result
    );
}

#[test]
fn test_not_writable_when_mut_required() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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

// ---------------------------------------------------------------------------
// QuasarError::ConstraintViolation (3004) — default constraint error
// ---------------------------------------------------------------------------

#[test]
fn test_constraint_violation_default_error() {
    let mollusk = setup();
    let target = Address::new_unique();
    let instruction: Instruction = ConstraintDefaultInstruction { target }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(target, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert_eq!(
        result.program_result,
        MolluskResult::Failure(ProgramError::Custom(3004))
    );
}

// ---------------------------------------------------------------------------
// QuasarError::HasOneMismatch (3005) — default has_one error
// ---------------------------------------------------------------------------

#[test]
fn test_has_one_mismatch_default_error() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(wrong_authority, 42);
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

#[test]
fn test_has_one_default_passes_when_matching() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(authority, 42);
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

// ---------------------------------------------------------------------------
// QuasarError::AddressMismatch (3012) — default address error
// ---------------------------------------------------------------------------

#[test]
fn test_address_mismatch_default_error() {
    let mollusk = setup();
    let wrong_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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

#[test]
fn test_address_default_passes_when_matching() {
    let mollusk = setup();
    let expected_addr = Address::new_from_array([88u8; 32]);
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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

// ---------------------------------------------------------------------------
// ProgramError::IllegalOwner — SystemAccount wrong owner
// ---------------------------------------------------------------------------

#[test]
fn test_system_account_owner_error() {
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

// ---------------------------------------------------------------------------
// ProgramError::IncorrectProgramId — Program<T> wrong address
// ---------------------------------------------------------------------------

#[test]
fn test_program_id_error() {
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
                owner: Address::default(),
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

// ---------------------------------------------------------------------------
// ProgramError::InvalidAccountData — Program<T> not executable
// ---------------------------------------------------------------------------

#[test]
fn test_program_not_executable_error() {
    let mollusk = setup();
    let system_id = Address::new_from_array([0u8; 32]);
    let instruction = Instruction {
        program_id: quasar_test_errors::ID,
        accounts: vec![AccountMeta::new_readonly(system_id, false)],
        data: vec![21],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(
            system_id,
            Account {
                lamports: 1_000_000,
                data: vec![],
                owner: system_id,
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

// ---------------------------------------------------------------------------
// ProgramError::AccountBorrowFailed — Duplicate account detection
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_account_error() {
    let mollusk = setup();
    let account_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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

// ---------------------------------------------------------------------------
// Address constraint with custom error (address = X @ CustomError)
// ---------------------------------------------------------------------------

#[test]
fn test_address_custom_error_success() {
    let mollusk = setup();
    let expected_addr = Address::new_from_array([99u8; 32]);
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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
    assert!(
        result.program_result.is_ok(),
        "address custom error should pass with correct address: {:?}",
        result.program_result
    );
}

#[test]
fn test_address_custom_error_wrong_address() {
    let mollusk = setup();
    let wrong_addr = Address::new_unique();
    let account_data = build_error_test_account_data(Address::new_unique(), 42);
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
        MolluskResult::Failure(ProgramError::Custom(104)),
        "address mismatch should return AddressCustom error (104)"
    );
}
