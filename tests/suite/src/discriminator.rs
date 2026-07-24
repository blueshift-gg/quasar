use {
    crate::compat::{Instruction, ProgramError, Pubkey},
    crate::helpers::*,
    quasar_test_misc::cpi::*,
};

// Happy paths.

#[test]
fn single_byte_valid() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(&ix, &[simple_account(account, authority, 42, 0)]);
    assert!(result.is_ok(), "single byte disc: {:?}", result.raw_result);
}

#[test]
fn multi_byte_valid() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = CheckMultiDiscInstruction { account }.into();
    let result = svm.process_instruction(&ix, &[multi_disc_account(account, 42)]);
    assert!(result.is_ok(), "multi byte disc: {:?}", result.raw_result);
}

// Error paths.

#[test]
fn single_byte_wrong() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 42];
    data[0] = 99; // wrong disc (expected 2)

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_err(), "wrong single-byte disc");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn single_byte_zero() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let data = vec![0u8; 42]; // disc = 0 (uninitialized)

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_err(), "zero disc");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn multi_byte_partial_match() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 10];
    data[0] = 1; // first byte correct
    data[1] = 0; // second byte wrong (expected 2)

    let ix: Instruction = CheckMultiDiscInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_err(), "partial disc match");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn multi_byte_reversed() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let mut data = vec![0u8; 10];
    data[0] = 2; // swapped
    data[1] = 1; // swapped

    let ix: Instruction = CheckMultiDiscInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_err(), "reversed disc");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn zero_length_data() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            1_000_000,
            vec![],
            quasar_test_misc::ID,
        )],
    );
    assert!(result.is_err(), "zero length data");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

#[test]
fn disc_only_no_fields() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let data = vec![2u8]; // just the disc, no struct fields

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_err(), "disc only, no fields");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

#[test]
fn oversized_data_valid() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let mut data = vec![0u8; 10_000];
    data[0] = 2; // correct disc
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&42u64.to_le_bytes());
    data[41] = 0; // bump

    let ix: Instruction = OwnerCheckInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(
            account,
            100_000_000,
            data,
            quasar_test_misc::ID,
        )],
    );
    assert!(
        result.is_ok(),
        "oversized data should be accepted: {:?}",
        result.raw_result
    );
}

// NoDiscAccount with unsafe_no_disc has no discriminator check.

#[test]
fn no_disc_init_success() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"nodisc", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitNoDiscInstruction { payer, value: 42 }.into();

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_ok(), "no_disc init: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account");
    assert_eq!(acc.data.len(), 40, "no-disc size (no disc prefix)");
    assert_eq!(&acc.data[0..32], payer.as_ref(), "authority");
    assert_eq!(&acc.data[32..40], &42u64.to_le_bytes(), "value");
}

#[test]
fn no_disc_read_after_init() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"nodisc", payer.as_ref()], &quasar_test_misc::ID);

    // Init first
    let ix1: Instruction = InitNoDiscInstruction { payer, value: 99 }.into();
    let r1 = svm.process_instruction(&ix1, &[rich_signer_account(payer), empty_account(account)]);
    assert!(r1.is_ok(), "init: {:?}", r1.raw_result);

    // Read through Deref-backed .authority and .value accessors.
    let ix2: Instruction = ReadNoDiscInstruction { account }.into();
    let r2 = svm.process_instruction(&ix2, &[]);
    assert!(r2.is_ok(), "read: {:?}", r2.raw_result);
}

#[test]
fn no_disc_any_data_accepted() {
    // Since unsafe_no_disc skips discriminator check, any 40+ byte data
    // owned by the program should pass validation.
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    // All 0xFF data has no valid discriminator, but unsafe_no_disc skips the check.
    let data = vec![0xFF; 40];
    let ix: Instruction = ReadNoDiscInstruction { account }.into();
    let result = svm.process_instruction(
        &ix,
        &[raw_account(account, 1_000_000, data, quasar_test_misc::ID)],
    );
    assert!(result.is_ok(), "any data accepted: {:?}", result.raw_result);
}
