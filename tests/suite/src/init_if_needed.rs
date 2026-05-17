use {
    crate::helpers::*,
    quasar_svm::{Account, Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

// Happy paths.

#[test]
fn new_account() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_ok(), "new account: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data[0], 1, "discriminator");
    assert_eq!(&acc.data[33..41], &42u64.to_le_bytes(), "value");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");
}

#[test]
fn existing_valid() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 99,
    }
    .into();

    // Already initialized with correct owner/disc/size
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            simple_account(account, payer, 42, bump),
        ],
    );
    assert!(result.is_ok(), "existing valid: {:?}", result.raw_result);
}

#[test]
fn existing_value_updated() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 99,
    }
    .into();

    let payer_lamports_before = 100_000_000_000u64;
    let account_lamports_before = 1_000_000u64;
    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: payer,
                lamports: payer_lamports_before,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            simple_account(account, payer, 42, bump),
        ],
    );
    assert!(result.is_ok(), "existing value: {:?}", result.raw_result);

    // Verify payer NOT charged (no init CPI happened)
    let payer_after = result.account(&payer).expect("payer");
    assert_eq!(
        payer_after.lamports, payer_lamports_before,
        "payer not charged"
    );

    // Verify account lamports unchanged
    let acc = result.account(&account).expect("account");
    assert_eq!(
        acc.lamports, account_lamports_before,
        "account lamports unchanged"
    );
}

#[test]
fn new_prefunded() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // System-owned with lamports uses the pre-funded init path.
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            prefunded_account(account, 500_000),
        ],
    );
    assert!(result.is_ok(), "new prefunded: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data[0], 1, "discriminator");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");
}

// Error paths for the existing branch.

#[test]
fn wrong_owner() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Owned by random program (not system, not ours)
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            raw_account(account, 1_000_000, vec![1u8; 42], Pubkey::new_unique()),
        ],
    );
    assert!(result.is_err(), "should reject wrong owner");
}

#[test]
fn wrong_discriminator() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Correct owner, wrong discriminator
    let mut data = vec![0u8; 42];
    data[0] = 99; // wrong disc
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            raw_account(account, 1_000_000, data, quasar_test_misc::ID),
        ],
    );
    assert!(result.is_err(), "should reject wrong discriminator");
    result.assert_error(ProgramError::InvalidAccountData);
}

#[test]
fn data_too_small() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Correct owner + disc but data too small
    let mut data = vec![0u8; 10]; // too small (42 needed)
    data[0] = 1; // correct disc
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            raw_account(account, 1_000_000, data, quasar_test_misc::ID),
        ],
    );
    assert!(result.is_err(), "should reject undersized data");
    result.assert_error(ProgramError::AccountDataTooSmall);
}

#[test]
fn not_writable() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let mut ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();
    ix.accounts[1].is_writable = false;

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_err(), "should reject non-writable");
    result.assert_error(ProgramError::Immutable);
}

// Error paths for the new branch.

#[test]
fn payer_insufficient_funds() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: payer,
                lamports: 1,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            empty_account(account),
        ],
    );
    assert!(result.is_err(), "should reject insufficient funds");
}

// Front-running scenario.

#[test]
fn front_running_attacker_data() {
    // Attacker inits account with correct owner+disc but wrong field data
    // before legitimate user calls init_if_needed.
    //
    // This instruction declares no extra semantic constraints on the existing
    // account beyond the framework's structural checks, so the existing-account
    // branch is accepted and the handler must repair the state itself.
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let attacker = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 99,
    }
    .into();

    // Account already initialized by "attacker" with correct owner, disc, and
    // size, but authority = attacker (not payer).
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            simple_account(account, attacker, 666, bump),
        ],
    );
    // Succeeds because existing account passes the structural checks for this
    // particular instruction (owner+disc+size OK, no extra declarative
    // constraints).
    assert!(result.is_ok(), "front-run: {:?}", result.raw_result);

    // Handler always calls set_inner(), so authority is overwritten to payer
    // and value to 99; the attacker's data does not persist.
    let acc = result.account(&account).expect("account");
    assert_eq!(
        &acc.data[1..33],
        payer.as_ref(),
        "authority overwritten to payer"
    );
    assert_eq!(
        &acc.data[33..41],
        &99u64.to_le_bytes(),
        "value overwritten to 99"
    );
}
