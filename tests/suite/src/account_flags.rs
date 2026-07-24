use {
    crate::compat::{Instruction, ProgramError, Pubkey},
    crate::helpers::*,
    quasar_test_misc::cpi::*,
};

// Signer.

#[test]
fn signer_success() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();

    let ix: Instruction = SignerCheckInstruction { signer }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_ok(), "signer: {:?}", result.raw_result);
}

#[test]
fn signer_not_signer() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();

    let mut ix: Instruction = SignerCheckInstruction { signer }.into();
    ix.accounts[0].is_signer = false;

    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_err(), "not signer");
    result.assert_error(ProgramError::MissingRequiredSignature);
}

// Mut.

#[test]
fn mut_success() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let ix: Instruction = MutCheckInstruction {
        account,
        new_value: 99,
    }
    .into();
    let result =
        svm.process_instruction(&ix, &[simple_account(account, Pubkey::new_unique(), 42, 0)]);
    assert!(result.is_ok(), "mut: {:?}", result.raw_result);
}

#[test]
fn mut_write_persists() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = MutCheckInstruction {
        account,
        new_value: 99,
    }
    .into();
    let result = svm.process_instruction(&ix, &[simple_account(account, authority, 42, 0)]);
    assert!(result.is_ok(), "mut write: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account");
    assert_eq!(
        &acc.data[33..41],
        &99u64.to_le_bytes(),
        "written value persisted"
    );
}

#[test]
fn mut_not_writable() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();

    let mut ix: Instruction = MutCheckInstruction {
        account,
        new_value: 99,
    }
    .into();
    ix.accounts[0].is_writable = false;

    let result =
        svm.process_instruction(&ix, &[simple_account(account, Pubkey::new_unique(), 42, 0)]);
    assert!(result.is_err(), "not writable");
    result.assert_error(ProgramError::Immutable);
}

// Combined signer + mut.

#[test]
fn signer_and_mut_success() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let signer = Pubkey::new_unique();

    let ix: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 99,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, Pubkey::new_unique(), 42, 0),
            signer_account(signer),
        ],
    );
    assert!(result.is_ok(), "signer+mut: {:?}", result.raw_result);
}

#[test]
fn signer_and_mut_missing_signer() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let signer = Pubkey::new_unique();

    let mut ix: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 99,
    }
    .into();
    ix.accounts[1].is_signer = false;

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, Pubkey::new_unique(), 42, 0),
            signer_account(signer),
        ],
    );
    assert!(result.is_err(), "missing signer");
    result.assert_error(ProgramError::MissingRequiredSignature);
}

#[test]
fn signer_and_mut_not_writable() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let signer = Pubkey::new_unique();

    let mut ix: Instruction = SignerAndMutCheckInstruction {
        account,
        signer,
        new_value: 99,
    }
    .into();
    ix.accounts[0].is_writable = false;

    let result = svm.process_instruction(
        &ix,
        &[
            simple_account(account, Pubkey::new_unique(), 42, 0),
            signer_account(signer),
        ],
    );
    assert!(result.is_err(), "not writable");
    result.assert_error(ProgramError::Immutable);
}

// Dup-allowed path, #[account(dup)], separate codegen from nodup.

#[test]
fn dup_readonly_same_account_succeeds() {
    // HeaderDupReadonly: source=Signer, destination=dup readonly UncheckedAccount
    // Same pubkey for both should succeed because destination is an explicit
    // read-only alias role.
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();

    let ix: Instruction = quasar_test_errors::cpi::HeaderDupReadonlyInstruction {
        source: account,
        destination: account,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(account)]);
    assert!(
        result.is_ok(),
        "dup readonly same account: {:?}",
        result.raw_result
    );
}

#[test]
fn dup_signer_same_account_succeeds() {
    // HeaderDupSigner: payer=mut Signer, authority=dup Signer
    // Same pubkey for both should succeed because authority has #[account(dup)]
    let mut svm = svm_errors();
    let account = Pubkey::new_unique();

    let ix: Instruction = quasar_test_errors::cpi::HeaderDupSignerInstruction {
        payer: account,
        authority: account,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(account)]);
    assert!(
        result.is_ok(),
        "dup signer same account: {:?}",
        result.raw_result
    );
}

#[test]
fn three_accounts_no_dup_rejects_same() {
    // ThreeAccountsDup: Signer + mut UncheckedAccount + UncheckedAccount
    // No #[account(dup)], so second==third must be rejected by nodup check.
    let mut svm = svm_errors();
    let signer = Pubkey::new_unique();
    let shared = Pubkey::new_unique();

    let ix: Instruction = quasar_test_errors::cpi::ThreeAccountsDupInstruction {
        first: signer,
        second: shared,
        third: shared, // same as second
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(signer),
            signer_account(shared), // just needs some account
        ],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(crate::compat::ProgramError::Runtime(
        "AccountBorrowFailed".into(),
    ));
}

// Double mut with two separate &mut fields in one instruction.

#[test]
fn double_mut_distinct_accounts() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    let authority = Pubkey::new_unique();

    let ix: Instruction = DoubleMutCheckInstruction {
        signer,
        account_a: a,
        account_b: b,
    }
    .into();
    let result = svm.process_instruction(
        &ix,
        &[
            signer_account(signer),
            simple_account(a, authority, 42, 0),
            simple_account(b, authority, 99, 0),
        ],
    );
    assert!(result.is_ok(), "double mut: {:?}", result.raw_result);
}
