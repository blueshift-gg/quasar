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

    let ix: Instruction = CloseAccountInstruction { authority, account }.into();

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

    let ix: Instruction = CloseAccountInstruction { authority, account }.into();

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
    // Same as lamports_transferred but with explicit large balances
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let x = 50_000_000u64;
    let y = 3_000_000u64;

    let ix: Instruction = CloseAccountInstruction { authority, account }.into();

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

    let ix: Instruction = CloseAccountInstruction {
        authority: wrong_authority,
        account,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(wrong_authority),
            simple_account(account, real_authority, 42, bump),
        ],
    );
    assert!(result.is_err(), "should reject wrong authority");
    // v3: address verification runs before has_one, so the PDA mismatch
    // (wrong authority in seeds) is caught first.
    result.assert_error(ProgramError::InvalidSeeds);
}

#[test]
fn authority_not_signer() {
    let mut svm = svm_misc();
    let authority = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    let mut ix: Instruction = CloseAccountInstruction { authority, account }.into();
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
