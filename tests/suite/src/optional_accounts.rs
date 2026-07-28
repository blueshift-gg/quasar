use {
    crate::compat::{Instruction, ProgramError, Pubkey},
    crate::helpers::*,
    quasar_test_misc::cpi::*,
};

#[test]
fn some_valid() {
    let mut svm = svm_misc();
    let required = Pubkey::new_unique();
    let optional = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = OptionalAccountInstruction { required, optional }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(required, authority, 42, 0),
            simple_account(optional, authority, 99, 0),
        ],
    );
    assert!(result.is_ok(), "both present: {:?}", result.raw_result);
}

#[test]
fn none_sentinel() {
    let mut svm = svm_misc();
    let required = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    // Sentinel = program ID for None
    let sentinel = quasar_test_misc::ID;
    let ix: Instruction = OptionalAccountInstruction {
        required,
        optional: sentinel,
    }
    .into();

    let result = svm.process_instruction(&ix, &[simple_account(required, authority, 42, 0)]);
    assert!(result.is_ok(), "sentinel none: {:?}", result.raw_result);
}

#[test]
fn has_one_some_valid() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = OptionalHasOneInstruction { authority, account }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, authority, 42, 0),
        ],
    );
    assert!(
        result.is_ok(),
        "has_one some valid: {:?}",
        result.raw_result
    );
}

#[test]
fn has_one_none_skipped() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let sentinel = quasar_test_misc::ID;

    let ix: Instruction = OptionalHasOneInstruction {
        authority,
        account: sentinel,
    }
    .into();

    let result = svm.process_instruction(&ix, &[signer_account(authority)]);
    assert!(
        result.is_ok(),
        "has_one none skipped: {:?}",
        result.raw_result
    );
}

#[test]
fn has_one_some_wrong_authority() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let account = Pubkey::new_unique();

    let ix: Instruction = OptionalHasOneInstruction { authority, account }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(authority),
            simple_account(account, wrong_authority, 42, 0), // wrong authority stored
        ],
    );
    assert!(
        result.is_err(),
        "should reject wrong authority on present optional"
    );
    result.assert_error(ProgramError::Custom(3005)); // HasOneMismatch
}

// Validation still runs when present, not sentinel.

#[test]
fn some_wrong_owner() {
    // Optional account is present (not sentinel) but owned by wrong program.
    // Proves Optional doesn't skip validation when the account IS present.
    let mut svm = svm_misc();
    let required = Pubkey::new_unique();
    let optional = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = OptionalAccountInstruction { required, optional }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(required, authority, 42, 0),
            raw_account(
                optional,
                1_000_000,
                build_simple_data(authority, 99, 0),
                Pubkey::new_unique(), // wrong owner
            ),
        ],
    );
    assert!(result.is_err(), "wrong owner on present optional");
}

#[test]
fn some_wrong_discriminator() {
    // Optional account is present but has wrong discriminator.
    let mut svm = svm_misc();
    let required = Pubkey::new_unique();
    let optional = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = OptionalAccountInstruction { required, optional }.into();

    let mut bad_data = vec![0u8; 42];
    bad_data[0] = 99; // wrong disc
    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(required, authority, 42, 0),
            raw_account(optional, 1_000_000, bad_data, quasar_test_misc::ID),
        ],
    );
    assert!(result.is_err(), "wrong disc on present optional");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn multiple_mut_optional_none_sentinels() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let sentinel = quasar_test_misc::ID;

    let ix: Instruction = OptionalMutAccountsInstruction {
        authority,
        first: sentinel,
        second: sentinel,
        third: sentinel,
    }
    .into();

    let result = svm.process_instruction(&ix, &[signer_account(authority)]);
    assert!(
        result.is_ok(),
        "all mut optionals as None sentinel should parse: {:?}",
        result.raw_result
    );
}
