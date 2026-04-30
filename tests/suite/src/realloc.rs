use {
    crate::helpers::*,
    quasar_svm::{Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

fn setup_account(svm: &mut quasar_svm::QuasarSvm) -> (Pubkey, Pubkey, Pubkey) {
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Init first
    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();
    let r = svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(r.is_ok(), "setup init: {:?}", r.raw_result);
    (payer, account, quasar_svm::system_program::ID)
}

fn realloc(
    svm: &mut quasar_svm::QuasarSvm,
    account: Pubkey,
    payer: Pubkey,
    new_space: u64,
) -> quasar_svm::ExecutionResult {
    let ix: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program: quasar_svm::system_program::ID,
        new_space,
    }
    .into();
    svm.process_instruction(&ix, &[])
}

#[test]
fn grow() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);
    let result = realloc(&mut svm, account, payer, 100);
    assert!(result.is_ok(), "grow: {:?}", result.raw_result);
    let acc = result.account(&account).expect("account");
    assert_eq!(acc.data.len(), 100, "new length");
}

#[test]
fn grow_preserves_data() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);

    // Read original data
    let orig = svm.get_account(&account).expect("account").data.clone();

    let result = realloc(&mut svm, account, payer, 100);
    assert!(result.is_ok(), "grow: {:?}", result.raw_result);
    let acc = result.account(&account).expect("account");
    assert_eq!(&acc.data[..42], &orig[..], "original 42 bytes preserved");
}

#[test]
fn shrink() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);

    // First grow
    let r1 = realloc(&mut svm, account, payer, 100);
    assert!(r1.is_ok(), "grow: {:?}", r1.raw_result);

    // Then shrink
    let r2 = realloc(&mut svm, account, payer, 42);
    assert!(r2.is_ok(), "shrink: {:?}", r2.raw_result);
    let acc = r2.account(&account).expect("account");
    assert_eq!(acc.data.len(), 42, "shrunk back");
}

#[test]
fn same_size_noop() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);
    let result = realloc(&mut svm, account, payer, 42);
    assert!(result.is_ok(), "noop: {:?}", result.raw_result);
    let acc = result.account(&account).expect("account");
    assert_eq!(acc.data.len(), 42, "unchanged");
}

#[test]
fn grow_large() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);
    let result = realloc(&mut svm, account, payer, 10_000);
    assert!(result.is_ok(), "grow large: {:?}", result.raw_result);
    let acc = result.account(&account).expect("account");
    assert_eq!(acc.data.len(), 10_000, "large size");
}

#[test]
fn grow_zeroes_new_region() {
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);

    // Grow to 100
    let r1 = realloc(&mut svm, account, payer, 100);
    assert!(r1.is_ok());

    // Shrink to 42
    let r2 = realloc(&mut svm, account, payer, 42);
    assert!(r2.is_ok());

    // Grow to 100 again — new region should be zeroed
    let r3 = realloc(&mut svm, account, payer, 100);
    assert!(r3.is_ok(), "re-grow: {:?}", r3.raw_result);
    let acc = r3.account(&account).expect("account");
    assert!(
        acc.data[42..100].iter().all(|&b| b == 0),
        "re-grown region must be zeroed (no data leak)"
    );
}

#[test]
fn shrink_below_struct_rejects() {
    // Attempt to shrink below the struct's minimum size — the realloc handler
    // itself rejects this because the account data would be too small.
    let mut svm = svm_misc();
    let (payer, account, _) = setup_account(&mut svm);

    // Shrink to 4 bytes — handler rejects because SimpleAccount needs 42 bytes
    let r1 = realloc(&mut svm, account, payer, 4);
    assert!(r1.is_err(), "shrink below struct size should fail");
    r1.assert_error(ProgramError::AccountDataTooSmall);
}
