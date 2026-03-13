use {
    mollusk_svm::Mollusk, quasar_test_misc::client::*, solana_account::Account,
    solana_address::Address, solana_instruction::Instruction,
};

fn build_simple_account_data(authority: Address, value: u64, bump: u8) -> Vec<u8> {
    let mut data = vec![0u8; 42];
    data[0] = 1; // SimpleAccount discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data[41] = bump;
    data
}

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_misc::ID,
        "../../target/deploy/quasar_test_misc",
    )
}

// ============================================================================
// Constraint: has_one (tests 13-16)
// ============================================================================

#[test]
fn test_has_one_success() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_ok(),
        "has_one should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_has_one_wrong_authority() {
    let mollusk = setup();

    let real_authority = Address::new_unique();
    let fake_authority = Address::new_unique();
    let fake_authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", fake_authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(real_authority, 42, bump), // Authority stored = real
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction {
        authority: fake_authority, // But passing fake
        account,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (fake_authority, fake_authority_account),
            (account, account_obj),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail with wrong authority"
    );
}

#[test]
fn test_has_one_zeroed_authority() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    // Stored authority is all-zero
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::default(), 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when stored authority is all-zero"
    );
}

#[test]
fn test_has_one_single_bit_diff() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    // Create authority that differs by 1 bit
    let mut wrong_bytes = authority.to_bytes();
    wrong_bytes[0] ^= 1;
    let wrong_authority = Address::new_from_array(wrong_bytes);

    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(wrong_authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when authority differs by 1 bit"
    );
}

// ============================================================================
// Constraint: address (tests 17-19)
// ============================================================================

#[test]
fn test_address_success() {
    let mollusk = setup();

    let target = quasar_test_misc::EXPECTED_ADDRESS;
    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "address check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_address_wrong() {
    let mollusk = setup();

    let wrong_target = Address::new_unique();
    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction {
        target: wrong_target,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(wrong_target, target_account)]);

    assert!(
        result.program_result.is_err(),
        "address check should fail with wrong address"
    );
}

#[test]
fn test_address_with_constant() {
    let mollusk = setup();

    // Verify that the const address is the expected value
    let target = Address::new_from_array([42u8; 32]);
    assert_eq!(target, quasar_test_misc::EXPECTED_ADDRESS);

    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "const address should work: {:?}",
        result.program_result
    );
}

// ============================================================================
// Constraint: signer (tests 20-22)
// ============================================================================

#[test]
fn test_signer_success() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SignerCheckInstruction { signer }.into();

    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    assert!(
        result.program_result.is_ok(),
        "signer check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_signer_not_signer() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = SignerCheckInstruction { signer }.into();
    instruction.accounts[0].is_signer = false;

    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    assert!(
        result.program_result.is_err(),
        "signer check should fail when not signer"
    );
}

// ============================================================================
// Constraint: owner (tests 22-24)
// ============================================================================

#[test]
fn test_owner_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID, // Correct owner
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "owner check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_owner_wrong_program() {
    let mollusk = setup();

    let account = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: wrong_owner, // Wrong owner
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "owner check should fail with wrong program"
    );
}

#[test]
fn test_owner_system_program() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: Address::default(), // System program (uninitialized)
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "owner check should fail when owned by system program"
    );
}

// ============================================================================
// Constraint: mut (tests 26-28)
// ============================================================================

#[test]
fn test_mut_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "mut check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_mut_not_writable() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 100,
    }
    .into();

    // Make account read-only
    instruction.accounts[0] = solana_instruction::AccountMeta::new_readonly(account, false);

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "mut check should fail when account is not writable"
    );
}

#[test]
fn test_mut_writes_persist() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 999,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(result.program_result.is_ok());

    let data = &result.resulting_accounts[0].1.data;
    assert_eq!(
        &data[33..41],
        &999u64.to_le_bytes(),
        "written value should persist"
    );
}

// ============================================================================
// SystemAccount (tests 33-34)
// ============================================================================

#[test]
fn test_system_account_success() {
    let mollusk = setup();

    let target = Address::new_unique();
    let target_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SystemAccountCheckInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "system account check should pass for system-owned account: {:?}",
        result.program_result
    );
}

#[test]
fn test_system_account_wrong_owner() {
    let mollusk = setup();

    let target = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let target_account = Account {
        lamports: 1_000_000,
        data: Vec::new(),
        owner: wrong_owner,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SystemAccountCheckInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_err(),
        "system account check should fail when owner is not system program"
    );
}

// ============================================================================
// Constraint Check
// ============================================================================

#[test]
fn test_constraint_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 100, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "constraint should pass when value > 0: {:?}",
        result.program_result
    );
}

#[test]
fn test_constraint_fail_zero_value() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 0, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "constraint should fail when value == 0"
    );
}

// ============================================================================
// Optional Account with has_one constraint (discriminator 19)
// ============================================================================

#[test]
fn test_optional_has_one_some_valid() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_simple_account_data(authority, 42, 0);

    let instruction: Instruction = OptionalHasOneInstruction {
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
                    owner: quasar_test_misc::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "optional has_one with valid authority should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_optional_has_one_some_wrong() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_simple_account_data(wrong_authority, 42, 0);

    let instruction: Instruction = OptionalHasOneInstruction {
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
                    owner: quasar_test_misc::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "optional has_one with wrong authority should fail"
    );
}

#[test]
fn test_optional_has_one_none() {
    let mollusk = setup();
    let authority = Address::new_unique();

    let instruction: Instruction = OptionalHasOneInstruction {
        authority,
        account: quasar_test_misc::ID,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, Account::new(1_000_000, 0, &Address::default()))],
    );

    assert!(
        result.program_result.is_ok(),
        "optional has_one with None should pass (constraint skipped): {:?}",
        result.program_result
    );
}

// ============================================================================
// has_one: default address stored
// ============================================================================

#[test]
fn test_has_one_authority_is_default_address() {
    let mollusk = setup();

    let authority = Address::default();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let real_authority = Address::new_unique();
    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(real_authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when passed authority is default but stored is different"
    );
}

#[test]
fn test_has_one_last_byte_diff() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut wrong_bytes = authority.to_bytes();
    wrong_bytes[31] ^= 0xFF;
    let wrong_authority = Address::new_from_array(wrong_bytes);

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(wrong_authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when authority differs by last byte"
    );
}

// ============================================================================
// address: off by one byte
// ============================================================================

#[test]
fn test_address_off_by_one_byte() {
    let mollusk = setup();

    let mut wrong_bytes = quasar_test_misc::EXPECTED_ADDRESS.to_bytes();
    wrong_bytes[31] ^= 1;
    let wrong_target = Address::new_from_array(wrong_bytes);

    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction {
        target: wrong_target,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(wrong_target, target_account)]);

    assert!(
        result.program_result.is_err(),
        "address check should fail when address differs by single byte"
    );
}

// ============================================================================
// owner: default address
// ============================================================================

#[test]
fn test_owner_is_default_address() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: Address::default(),
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "owner check should fail when owner is Address::default()"
    );
}

// ============================================================================
// signer: exact error type
// ============================================================================

#[test]
fn test_signer_not_signer_returns_missing_sig() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = SignerCheckInstruction { signer }.into();
    instruction.accounts[0].is_signer = false;

    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    assert_eq!(
        result.program_result,
        mollusk_svm::result::ProgramResult::Failure(
            quasar_core::prelude::ProgramError::MissingRequiredSignature
        )
    );
}

#[test]
fn test_signer_account_also_writable() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 77,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, account_obj), (signer, signer_account)],
    );

    assert!(
        result.program_result.is_ok(),
        "signer + mut combo should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// mut: large data persists
// ============================================================================

#[test]
fn test_mut_large_data_persists() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: u64::MAX,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(result.program_result.is_ok());

    let data = &result.resulting_accounts[0].1.data;
    assert_eq!(
        &data[33..41],
        &u64::MAX.to_le_bytes(),
        "u64::MAX should persist after mut write"
    );
}

#[test]
fn test_mut_not_writable_returns_immutable() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 100,
    }
    .into();

    instruction.accounts[0] = solana_instruction::AccountMeta::new_readonly(account, false);

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert_eq!(
        result.program_result,
        mollusk_svm::result::ProgramResult::Failure(quasar_core::prelude::ProgramError::Immutable)
    );
}

// ============================================================================
// Combined constraints: signer + mut
// ============================================================================

#[test]
fn test_combined_signer_and_mut() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 55,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, account_obj), (signer, signer_account)],
    );

    assert!(
        result.program_result.is_ok(),
        "combined signer + mut should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_combined_signer_and_mut_missing_signer() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 55,
    }
    .into();

    instruction.accounts[1].is_signer = false;

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, account_obj), (signer, signer_account)],
    );

    assert!(
        result.program_result.is_err(),
        "combined signer + mut should fail when signer flag missing"
    );
}

#[test]
fn test_combined_signer_and_mut_not_writable() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 55,
    }
    .into();

    instruction.accounts[0] = solana_instruction::AccountMeta::new_readonly(account, false);

    let result = mollusk.process_instruction(
        &instruction,
        &[(account, account_obj), (signer, signer_account)],
    );

    assert!(
        result.program_result.is_err(),
        "combined signer + mut should fail when account not writable"
    );
}

// ============================================================================
// Combined constraints: has_one + owner (Account<T> checks both)
// ============================================================================

#[test]
fn test_combined_has_one_and_owner_success() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(authority, 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_ok(),
        "has_one + owner should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_combined_has_one_and_owner_wrong_authority() {
    let mollusk = setup();

    let real_authority = Address::new_unique();
    let fake_authority = Address::new_unique();
    let fake_authority_account = Account::new(1_000_000, 0, &Address::default());

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(real_authority, 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = HasOneAndOwnerCheckInstruction {
        authority: fake_authority,
        account,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (fake_authority, fake_authority_account),
            (account, account_obj),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "has_one + owner should fail with wrong authority"
    );
}

#[test]
fn test_combined_has_one_and_owner_wrong_owner() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(authority, 42, 0),
        owner: Address::new_unique(),
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one + owner should fail with wrong program owner"
    );
}

// ============================================================================
// Constraint with custom error
// ============================================================================

#[test]
fn test_constraint_custom_error_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 100, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCustomErrorInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "custom error constraint should pass when value > 0: {:?}",
        result.program_result
    );
}

#[test]
fn test_constraint_custom_error_fails_with_custom_code() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 0, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCustomErrorInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert_eq!(
        result.program_result,
        mollusk_svm::result::ProgramResult::Failure(quasar_core::prelude::ProgramError::Custom(2)),
        "custom error constraint should return TestError::CustomConstraint (2)"
    );
}

// ============================================================================
// Adversarial Tests — Attacker-Controlled Inputs
// ============================================================================

/// Pass an account with correct disc and owner but has_one authority is the
/// account's own address (self-referential). This shouldn't match unless the
/// stored authority literally equals the account address, which is extremely
/// unlikely with new_unique().
#[test]
fn test_adversarial_has_one_self_referential() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    // Store the ACCOUNT ADDRESS as authority — attacker tries self-reference
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(account, 42, bump), // stored authority = account itself
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    // The passed authority != stored authority (account addr), so this should fail
    assert!(
        result.program_result.is_err(),
        "has_one with self-referential stored authority must fail when passed authority differs"
    );
}

/// Constraint check with u64::MAX value — tests that the constraint
/// `account.value > 0` handles extreme values correctly.
#[test]
fn test_adversarial_constraint_u64_max_value() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), u64::MAX, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "constraint value > 0 should pass with u64::MAX: {:?}",
        result.program_result
    );
}

/// has_one + owner combined: account has correct disc and correct authority,
/// but owner is Address::default() (system program). The owner check in
/// Account<T> should catch this even though has_one passes.
#[test]
fn test_adversarial_has_one_correct_authority_system_owner() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(authority, 42, 0), // correct authority stored
        owner: Address::default(),                         // but owned by system program!
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one with correct authority but system program owner must be rejected"
    );
}

/// Signer check: pass account with is_signer=true in the AccountMeta but
/// the actual account is program-owned (not a wallet). The runtime should
/// still accept this since is_signer is a flag, but it's an unusual pattern.
#[test]
fn test_adversarial_signer_on_program_owned_account() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account {
        lamports: 1_000_000,
        data: vec![0u8; 10],         // has data
        owner: quasar_test_misc::ID, // owned by program, not system
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SignerCheckInstruction { signer }.into();
    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    // The signer check only verifies is_signer flag, not ownership.
    // A program-owned account CAN be a signer if the SVM marks it as such.
    assert!(
        result.program_result.is_ok(),
        "signer check should pass regardless of account owner: {:?}",
        result.program_result
    );
}

/// Mut check: modify account value to 0 then try constraint check.
/// First mutate the value to 0, then run constraint_check which requires value
/// > 0. This is a cross-instruction attack within the same test.
#[test]
fn test_adversarial_mut_then_constraint_sequence() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 100, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Step 1: Mutate value to 0
    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 0,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);
    assert!(result.program_result.is_ok(), "mut to 0 should succeed");
    let mutated = result.resulting_accounts[0].1.clone();

    // Step 2: Now run constraint check (requires value > 0)
    let instruction: Instruction = ConstraintCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, mutated)]);

    assert!(
        result.program_result.is_err(),
        "constraint check should fail after mutating value to 0"
    );
}
