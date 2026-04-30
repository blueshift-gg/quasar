use {
    crate::helpers::*,
    quasar_svm::{Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

// ============================================================================
// has_one — default error
// ============================================================================

#[test]
fn has_one_success() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = UpdateHasOneInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, authority, 42, bump),
        ],
    );
    assert!(result.is_ok(), "has_one: {:?}", result.raw_result);
}

#[test]
fn has_one_mismatch() {
    let mut svm = svm_misc();
    let real_authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", real_authority.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = UpdateHasOneInstruction {
        authority: wrong_authority,
        account,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(wrong_authority),
            simple_account(account, real_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "has_one mismatch");
    // v3: address verification runs before has_one, so the PDA mismatch
    // (wrong authority in seeds) is caught first.
    result.assert_error(ProgramError::InvalidSeeds);
}

#[test]
fn has_one_zeroed_authority() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let zero_authority = Pubkey::default();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = UpdateHasOneInstruction { authority, account }.into();
    // Stored authority is zeroed
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, zero_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "zeroed stored authority should fail");
    result.assert_error(ProgramError::Custom(3005));
}

#[test]
fn has_one_single_bit_diff() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    // XOR bit 0 of stored authority
    let mut bad_bytes = authority.to_bytes();
    bad_bytes[0] ^= 1;
    let bad_authority = Pubkey::from(bad_bytes);

    let ix: Instruction = UpdateHasOneInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, bad_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "single bit diff");
    result.assert_error(ProgramError::Custom(3005));
}

#[test]
fn has_one_last_byte_diff() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    // XOR byte 31
    let mut bad_bytes = authority.to_bytes();
    bad_bytes[31] ^= 0xFF;
    let bad_authority = Pubkey::from(bad_bytes);

    let ix: Instruction = UpdateHasOneInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, bad_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "last byte diff");
    result.assert_error(ProgramError::Custom(3005));
}

#[test]
fn has_one_default_passed() {
    let mut svm = svm_misc();
    let real_authority = Pubkey::new_unique();
    let default_authority = Pubkey::default();
    let (account, bump) = Pubkey::find_program_address(
        &[b"simple", default_authority.as_ref()],
        &quasar_test_misc::ID,
    );

    // Passed authority = default, stored = real
    let ix: Instruction = UpdateHasOneInstruction {
        authority: default_authority,
        account,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(default_authority),
            simple_account(account, real_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "default authority passed");
    result.assert_error(ProgramError::Custom(3005));
}

// ============================================================================
// has_one — custom error (via test-errors crate)
// ============================================================================

#[test]
fn has_one_custom_success() {
    let mut svm = svm_errors();
    let authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::HasOneCustomInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(account, authority, 42),
        ],
    );
    assert!(
        result.is_ok(),
        "has_one custom success: {:?}",
        result.raw_result
    );
}

#[test]
fn has_one_custom_mismatch() {
    let mut svm = svm_errors();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::HasOneCustomInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(account, wrong_authority, 42),
        ],
    );
    assert!(result.is_err(), "has_one custom mismatch");
    result.assert_error(ProgramError::Custom(0)); // TestError::Hello
}

// ============================================================================
// address — default error
// ============================================================================

#[test]
fn address_success() {
    let mut svm = svm_misc();
    let expected: Pubkey = Pubkey::from([42u8; 32]); // EXPECTED_ADDRESS in test-misc

    let ix: Instruction = UpdateAddressInstruction { target: expected }.into();
    let result = svm.process_instruction(
        &ix,
        &[simple_account(expected, Pubkey::new_unique(), 42, 0)],
    );
    assert!(result.is_ok(), "address match: {:?}", result.raw_result);
}

#[test]
fn address_mismatch() {
    let mut svm = svm_misc();
    let wrong = Pubkey::new_unique();

    let ix: Instruction = UpdateAddressInstruction { target: wrong }.into();
    let result =
        svm.process_instruction(&ix, &[simple_account(wrong, Pubkey::new_unique(), 42, 0)]);
    assert!(result.is_err(), "address mismatch");
    result.assert_error(ProgramError::Custom(3012)); // AddressMismatch
}

// ============================================================================
// address — custom error (via test-errors crate)
// ============================================================================

#[test]
fn address_custom_success() {
    let mut svm = svm_errors();
    let expected: Pubkey = Pubkey::from([99u8; 32]); // EXPECTED_ADDR in test-errors

    let ix: Instruction =
        quasar_test_errors::cpi::AddressCustomErrorInstruction { target: expected }.into();
    let result = svm.process_instruction(
        &ix,
        &[error_test_account(expected, Pubkey::new_unique(), 42)],
    );
    assert!(result.is_ok(), "address custom: {:?}", result.raw_result);
}

#[test]
fn address_custom_mismatch() {
    let mut svm = svm_errors();
    let wrong = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::AddressCustomErrorInstruction { target: wrong }.into();
    let result =
        svm.process_instruction(&ix, &[error_test_account(wrong, Pubkey::new_unique(), 42)]);
    assert!(result.is_err(), "address custom mismatch");
    result.assert_error(ProgramError::Custom(104)); // TestError::AddressCustom
}

// ============================================================================
// constraint — default error
// ============================================================================

#[test]
fn constraint_success() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = ConstraintCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, Pubkey::new_unique(), 100, 0), // value > 0
        ],
    );
    assert!(result.is_ok(), "constraint pass: {:?}", result.raw_result);
}

#[test]
fn constraint_fail() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = ConstraintCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, Pubkey::new_unique(), 0, 0), // value == 0
        ],
    );
    assert!(result.is_err(), "constraint fail");
    result.assert_error(ProgramError::Custom(3004)); // ConstraintViolation
}

// ============================================================================
// constraint — custom error
// ============================================================================

#[test]
fn constraint_custom_success() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = ConstraintCustomErrorInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[simple_account(account, Pubkey::new_unique(), 100, 0)],
    );
    assert!(
        result.is_ok(),
        "constraint custom pass: {:?}",
        result.raw_result
    );
}

#[test]
fn constraint_custom_fail() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = ConstraintCustomErrorInstruction { account }.into();
    let result =
        svm.process_instruction(&ix, &[simple_account(account, Pubkey::new_unique(), 0, 0)]);
    assert!(result.is_err(), "constraint custom fail");
    result.assert_error(ProgramError::Custom(2)); // TestError::CustomConstraint
}

// ============================================================================
// combined constraints
// ============================================================================

#[test]
fn has_one_and_owner_success() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, authority, 42, 0),
        ],
    );
    assert!(result.is_ok(), "combined: {:?}", result.raw_result);
}

#[test]
fn has_one_and_owner_wrong_authority() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, wrong_authority, 42, 0),
        ],
    );
    assert!(result.is_err(), "wrong authority");
    result.assert_error(ProgramError::Custom(3005)); // HasOneMismatch
}

#[test]
fn has_one_and_owner_wrong_owner() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = HasOneAndOwnerCheckInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            raw_account(
                account,
                1_000_000,
                build_simple_data(authority, 42, 0),
                Pubkey::new_unique(), // wrong owner
            ),
        ],
    );
    assert!(result.is_err(), "wrong owner");
    // SVM returns Runtime("IllegalOwner") for owner mismatches
}
