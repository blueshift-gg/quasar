use {
    crate::compat::{Account, Instruction, ProgramError, Pubkey},
    crate::helpers::*,
    quasar_test_errors::cpi::*,
};

// Happy paths.

#[test]
fn valid_account() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(&ix, &[error_test_account(account, authority, 42)]);
    assert!(result.is_ok(), "valid: {:?}", result.raw_result);
}

#[test]
fn valid_with_extra_data() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let mut data = build_error_test_data(authority, 42);
    data.extend_from_slice(&[0u8; 100]); // extra bytes

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            data,
            quasar_test_errors::ID,
        )],
    );
    // Current behavior: oversized data is accepted
    assert!(result.is_ok(), "extra data: {:?}", result.raw_result);
}

// Owner checks.

#[test]
fn wrong_owner() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            build_error_test_data(authority, 42),
            Pubkey::new_unique(), // wrong owner
        )],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime("IllegalOwner".into()));
    // SVM returns Runtime("IllegalOwner") for owner mismatches
}

#[test]
fn system_program_owner() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            build_error_test_data(authority, 42),
            crate::compat::system_program::ID,
        )],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime("IllegalOwner".into()));
}

// Discriminator checks.

#[test]
fn wrong_discriminator() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 41];
    data[0] = 99; // wrong disc

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            data,
            quasar_test_errors::ID,
        )],
    );
    assert!(result.is_err(), "wrong discriminator");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn zero_discriminator() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let data = vec![0u8; 41]; // disc = 0

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            data,
            quasar_test_errors::ID,
        )],
    );
    assert!(result.is_err(), "zero discriminator");
    result.assert_error(ProgramError::InvalidAccountData);
}

// Size checks.

#[test]
fn data_too_small() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 20]; // 41 needed
    data[0] = 1; // correct disc

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            data,
            quasar_test_errors::ID,
        )],
    );
    assert!(result.is_err(), "data too small");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

#[test]
fn empty_data() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            vec![],
            quasar_test_errors::ID,
        )],
    );
    assert!(result.is_err(), "empty data");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

#[test]
fn one_byte_short() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 40]; // 41 needed
    data[0] = 1;

    let ix: Instruction = AccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            data,
            quasar_test_errors::ID,
        )],
    );
    assert!(result.is_err(), "one byte short");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

// Duplicate detection.

#[test]
fn duplicate_same_address() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    // Two accounts with same address
    let ix: Instruction = TwoAccountsCheckInstruction {
        first: account,
        second: account,
    }
    .into();
    let result = svm.process_instruction(&ix, &[error_test_account(account, authority, 42)]);
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime(
        "AccountBorrowFailed".into(),
    ));
}

#[test]
fn two_distinct_accounts() {
    let mut svm = svm_errors();
    let first = Pubkey::new_unique();
    let second = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = TwoAccountsCheckInstruction { first, second }.into();
    let result = svm.process_instruction(
        &ix,
        &[
            error_test_account(first, authority, 42),
            error_test_account(second, authority, 99),
        ],
    );
    assert!(result.is_ok(), "distinct accounts: {:?}", result.raw_result);
}

// SystemAccount validation.

#[test]
fn system_account_success() {
    let mut svm = svm_errors();
    let target = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::SystemAccountCheckInstruction { account: target }.into();
    let result = svm.process_instruction(&ix, &[signer_account(target)]);
    assert!(result.is_ok(), "system account: {:?}", result.raw_result);
}

#[test]
fn system_account_wrong_owner() {
    let mut svm = svm_errors();
    let target = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::SystemAccountCheckInstruction { account: target }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(target, 1_000_000, vec![], Pubkey::new_unique())],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime("IllegalOwner".into()));
}

#[test]
fn system_account_owned_by_program() {
    let mut svm = svm_errors();
    let target = Pubkey::new_unique();

    let ix: Instruction =
        quasar_test_errors::cpi::SystemAccountCheckInstruction { account: target }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            target,
            1_000_000,
            vec![],
            quasar_test_errors::ID,
        )],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime("IllegalOwner".into()));
}

// Program<T> validation.

#[test]
fn program_success() {
    let mut svm = svm_errors();
    let ix: Instruction = ProgramCheckInstruction {}.into();

    let result = svm.process_instruction(&ix, &[]);
    assert!(result.is_ok(), "program check: {:?}", result.raw_result);
}

#[test]
fn program_wrong_id() {
    let mut svm = svm_errors();
    let wrong = Pubkey::new_unique();
    // The client fills the canonical Program<T> address; point the meta at a
    // wrong executable on purpose.
    let mut ix: Instruction = ProgramCheckInstruction {}.into();
    ix.accounts[0].pubkey = wrong;

    let result = svm.process_instruction(
        &ix,
        &[Account {
            address: wrong,
            lamports: 1_000_000,
            data: vec![],
            owner: Pubkey::default(),
            executable: true,
        }],
    );
    assert!(result.is_err(), "wrong program ID");
    result.assert_error(ProgramError::IncorrectProgramId);
}

#[test]
fn program_not_executable() {
    let mut svm = svm_errors();
    let system = crate::compat::system_program::ID;
    let ix: Instruction = ProgramCheckInstruction {}.into();

    let result = svm.process_instruction(
        &ix,
        &[Account {
            address: system,
            lamports: 1,
            data: vec![],
            owner: Pubkey::default(),
            executable: false,
        }],
    );
    assert!(result.is_err(), "not executable");
    result.assert_error(ProgramError::InvalidAccountData);
}

// UncheckedAccount verifies that no validation is applied.

#[test]
fn unchecked_any_owner_passes() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();

    let ix: Instruction = UncheckedAccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            vec![1, 2, 3],
            Pubkey::new_unique(),
        )],
    );
    assert!(
        result.is_ok(),
        "unchecked any owner: {:?}",
        result.raw_result
    );
}

#[test]
fn unchecked_empty_passes() {
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();

    let ix: Instruction = UncheckedAccountCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            0,
            vec![],
            crate::compat::system_program::ID,
        )],
    );
    assert!(result.is_ok(), "unchecked empty: {:?}", result.raw_result);
}
