use {
    crate::helpers::*,
    quasar_svm::{Account, Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

#[test]
fn success() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = CloseAccountInstruction { authority }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(authority),
            simple_account(account, authority, 42, bump),
        ],
    );
    assert!(result.is_ok(), "close: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account");
    assert_eq!(acc.lamports, 0, "lamports zeroed");
    assert_eq!(
        acc.owner,
        quasar_svm::system_program::ID,
        "owner reset to system"
    );
    assert!(
        acc.data.is_empty() || acc.data.iter().all(|&b| b == 0),
        "data cleared"
    );
}

#[test]
fn lamports_transferred() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let account_lamports = 2_000_000u64;
    let authority_lamports = 1_000_000u64;

    let ix: Instruction = CloseAccountInstruction { authority }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: authority,
                lamports: authority_lamports,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            Account {
                address: account,
                lamports: account_lamports,
                data: build_simple_data(authority, 42, bump),
                owner: quasar_test_misc::ID,
                executable: false,
            },
        ],
    );
    assert!(result.is_ok(), "close: {:?}", result.raw_result);

    let auth = result.account(&authority).expect("authority");
    assert_eq!(
        auth.lamports,
        authority_lamports + account_lamports,
        "authority receives exact lamports"
    );
}

#[test]
fn destination_balance_additive() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let x = 50_000_000u64;
    let y = 3_000_000u64;

    let ix: Instruction = CloseAccountInstruction { authority }.into();

    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: authority,
                lamports: x,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            Account {
                address: account,
                lamports: y,
                data: build_simple_data(authority, 42, bump),
                owner: quasar_test_misc::ID,
                executable: false,
            },
        ],
    );
    assert!(result.is_ok(), "close: {:?}", result.raw_result);

    let auth = result.account(&authority).expect("authority");
    assert_eq!(auth.lamports, x + y, "X + Y additive");
}

#[test]
fn wrong_authority() {
    let mut svm = svm_misc();
    let real_authority = Pubkey::new_unique();
    let wrong_authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", real_authority.as_ref()], &quasar_test_misc::ID);

    // The client derives the PDA from the passed authority; point the meta
    // back at the real account: wrong authority against it is the case under
    // test.
    let mut ix: Instruction = CloseAccountInstruction {
        authority: wrong_authority,
    }
    .into();
    ix.accounts[1].pubkey = account;

    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(wrong_authority),
            simple_account(account, real_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "should reject wrong authority");
    // has_one rejects before PDA verification.
    result.assert_error(ProgramError::Custom(
        quasar_lang::prelude::QuasarError::InvalidPda as u32,
    ));
}

#[test]
fn authority_not_signer() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let mut ix: Instruction = CloseAccountInstruction { authority }.into();
    ix.accounts[0].is_signer = false;

    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(authority),
            simple_account(account, authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "should reject non-signer authority");
    result.assert_error(ProgramError::MissingRequiredSignature);
}

#[test]
fn self_close_rejected_without_mutation() {
    let mut svm = svm_misc();
    let account = Pubkey::new_unique();
    let before = simple_account(account, Pubkey::new_unique(), 42, 7);

    let ix: Instruction = CloseAccountAliasInstruction {
        account,
        destination: account,
    }
    .into();

    let result = svm.process_instruction(&ix, core::slice::from_ref(&before));

    assert!(result.is_err(), "self-close should fail");
    result.assert_error(ProgramError::InvalidArgument);
    assert_eq!(result.account(&account), Some(&before));
}
