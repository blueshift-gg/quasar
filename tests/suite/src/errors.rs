use {
    crate::helpers::*,
    quasar_svm::{Instruction, ProgramError, Pubkey},
    quasar_test_errors::cpi::*,
};

#[test]
fn custom_error_code() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = CustomErrorInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    result.assert_error(ProgramError::Custom(0)); // TestError::Hello
}

#[test]
fn explicit_error_number() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = ExplicitErrorInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    result.assert_error(ProgramError::Custom(100)); // TestError::ExplicitNum
}

#[test]
fn require_false() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = RequireFalseInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    result.assert_error(ProgramError::Custom(101)); // TestError::RequireFailed
}

#[test]
fn program_error_propagation() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = ProgramErrorInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn require_eq_passes() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = RequireEqCheckInstruction { signer, a: 5, b: 5 }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_ok(), "eq passes: {:?}", result.raw_result);
}

#[test]
fn require_eq_fails() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = RequireEqCheckInstruction { signer, a: 1, b: 2 }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    result.assert_error(ProgramError::Custom(102)); // TestError::RequireEqFailed
}

#[test]
fn require_neq_passes() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = RequireNeqCheckInstruction { signer, a: 1, b: 2 }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_ok(), "neq passes: {:?}", result.raw_result);
}

#[test]
fn require_neq_fails() {
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();

    let ix: Instruction = RequireNeqCheckInstruction { signer, a: 5, b: 5 }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err());
    // require_neq uses the same error as require_eq in the test program
    result.assert_error(ProgramError::Custom(102));
}

// Default framework error codes, without custom error annotations.
// These exercise the separate codegen path for default versus custom errors.

#[test]
fn has_one_default_mismatch() {
    // has_one = authority without a custom error yields HasOneMismatch (3005).
    let mut svm = svm_errors();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = HasOneDefaultInstruction { authority, account }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            error_test_account(account, wrong_authority, 42),
        ],
    );
    assert!(result.is_err(), "has_one default mismatch");
    result.assert_error(ProgramError::Custom(3005)); // HasOneMismatch
}

#[test]
fn address_default_mismatch() {
    // address = EXPECTED_ADDR_DEFAULT without a custom error yields
    // AddressMismatch (3012).
    let mut svm = svm_errors();
    let wrong = Pubkey::new_unique();

    let ix: Instruction = AddressDefaultInstruction { target: wrong }.into();
    let result =
        svm.process_instruction(&ix, &[error_test_account(wrong, Pubkey::new_unique(), 42)]);
    assert!(result.is_err(), "address default mismatch");
    result.assert_error(ProgramError::Custom(3012)); // AddressMismatch
}

#[test]
fn constraint_default_fail() {
    // constraint = false without a custom error yields ConstraintViolation (3004).
    let mut svm = svm_errors();
    let target = Pubkey::new_unique();

    let ix: Instruction = ConstraintDefaultInstruction { target }.into();
    let result = svm.process_instruction(&ix, &[signer_account(target)]);
    assert!(result.is_err(), "constraint default fail");
    result.assert_error(ProgramError::Custom(3004)); // ConstraintViolation
}
