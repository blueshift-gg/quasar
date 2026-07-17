use {
    crate::helpers::*,
    quasar_svm::{Account, Instruction, ProgramError, Pubkey},
    quasar_test_misc::cpi::*,
};

// Happy paths.

#[test]
fn fresh_account() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_ok(), "fresh init: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data.len(), 42, "size");
    assert_eq!(acc.data[0], 2, "discriminator");
    assert_eq!(&acc.data[1..33], payer.as_ref(), "authority");
    assert_eq!(&acc.data[33..41], &42u64.to_le_bytes(), "value");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");
}

#[test]
fn prefunded_partial_rent() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 7,
    }
    .into();

    // Account has some lamports but less than rent-exempt minimum
    let prefund = 500_000u64;
    let payer_lamports = 100_000_000_000u64;
    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: payer,
                lamports: payer_lamports,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            prefunded_account(account, prefund),
        ],
    );
    assert!(result.is_ok(), "prefunded partial: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data[0], 2, "discriminator");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");

    // Verify payer was only charged the delta (rent - prefund), not full rent
    let payer_after = result.account(&payer).expect("payer");
    let charged = payer_lamports - payer_after.lamports;
    assert!(charged > 0, "payer should be charged something");
    assert!(
        charged < acc.lamports,
        "payer charged less than full rent (prefund covered the rest)"
    );
    assert_eq!(
        charged,
        acc.lamports - prefund,
        "payer charged exactly rent - prefund"
    );
}

#[test]
fn prefunded_excess_rent() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 7,
    }
    .into();

    // Account already has more than enough lamports
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            prefunded_account(account, 100_000_000),
        ],
    );
    assert!(result.is_ok(), "prefunded excess: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data[0], 2, "discriminator");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");
}

#[test]
fn after_close() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // First init
    let ix1: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();
    let r1 = svm.process_instruction(&ix1, &[rich_signer_account(payer), empty_account(account)]);
    assert!(r1.is_ok(), "first init: {:?}", r1.raw_result);

    // Close
    let ix2: Instruction = CloseAccountInstruction {
        authority: payer,
        account,
    }
    .into();
    let r2 = svm.process_instruction(&ix2, &[]);
    assert!(r2.is_ok(), "close: {:?}", r2.raw_result);

    // Re-init
    let ix3: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 99,
    }
    .into();
    let r3 = svm.process_instruction(&ix3, &[]);
    assert!(r3.is_ok(), "re-init: {:?}", r3.raw_result);

    let acc = r3.account(&account).expect("account exists");
    assert_eq!(&acc.data[33..41], &99u64.to_le_bytes(), "new value");
}

#[test]
fn space_override() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"spacetest", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = SpaceOverrideInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_ok(), "space override: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data.len(), 42, "struct-derived space");
}

#[test]
fn explicit_payer() {
    let mut svm = svm_misc();
    let funder = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"explicit", funder.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = ExplicitPayerInstruction {
        funder,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(funder), empty_account(account)]);
    assert!(result.is_ok(), "explicit payer: {:?}", result.raw_result);

    let acc = result.account(&account).expect("account exists");
    assert_eq!(acc.data[0], 4, "discriminator");
    assert_eq!(&acc.data[1..33], funder.as_ref(), "authority = funder");
}

// Error paths.

#[test]
fn already_initialized() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Account already owned by program with valid data
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            simple_account(account, payer, 42, bump),
        ],
    );
    assert!(result.is_err(), "should reject already-initialized");
    result.assert_error(ProgramError::AccountAlreadyInitialized);
}

#[test]
fn owned_by_other_program() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Account owned by random program
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            raw_account(account, 1_000_000, vec![0u8; 42], Pubkey::new_unique()),
        ],
    );
    assert!(
        result.is_err(),
        "should reject account owned by other program"
    );
    result.assert_error(ProgramError::AccountAlreadyInitialized);
}

#[test]
fn payer_not_signer() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let mut ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();
    ix.accounts[0].is_signer = false;

    let result =
        svm.process_instruction(&ix, &[rich_signer_account(payer), empty_account(account)]);
    assert!(result.is_err(), "should reject non-signer payer");
    result.assert_error(ProgramError::MissingRequiredSignature);
}

#[test]
fn payer_insufficient_funds() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // Payer with only 1 lamport
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
    // spl_token::TokenError::InsufficientFunds
    result.assert_error(quasar_svm::ProgramError::Custom(1));
}

#[test]
fn wrong_pda_seeds() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let wrong_account = Pubkey::new_unique(); // not a valid PDA

    let ix: Instruction = InitializeInstruction {
        payer,
        account: wrong_account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    let result = svm.process_instruction(
        &ix,
        &[rich_signer_account(payer), empty_account(wrong_account)],
    );
    result.assert_error(quasar_svm::ProgramError::Custom(
        quasar_lang::prelude::QuasarError::InvalidPda as u32,
    ));
}

#[test]
fn zero_data_owned_by_program() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 42,
    }
    .into();

    // All-zero data but owned by our program
    let result = svm.process_instruction(
        &ix,
        &[
            rich_signer_account(payer),
            raw_account(account, 1_000_000, vec![0u8; 42], quasar_test_misc::ID),
        ],
    );
    assert!(result.is_err(), "should reject zero-data owned by program");
    result.assert_error(ProgramError::AccountAlreadyInitialized);
}

// Pre-funded edge cases.

#[test]
fn prefunded_exact_no_topup() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let payer_lamports = 10_000_000_000u64;
    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 7,
    }
    .into();

    // Pre-fund with 10 SOL, well above rent-exempt minimum, so no transfer CPI.
    let result = svm.process_instruction(
        &ix,
        &[
            Account {
                address: payer,
                lamports: payer_lamports,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            prefunded_account(account, 100_000_000),
        ],
    );
    assert!(
        result.is_ok(),
        "prefunded no topup: {:?}",
        result.raw_result
    );

    // Verify payer was NOT charged because required top-up is zero.
    let payer_after = result.account(&payer).expect("payer");
    assert_eq!(payer_after.lamports, payer_lamports, "payer not charged");
}

#[test]
fn prefunded_one_lamport() {
    let mut svm = svm_misc();
    let payer = Pubkey::new_unique();
    let (account, _bump) =
        Pubkey::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let ix: Instruction = InitializeInstruction {
        payer,
        account,
        system_program: quasar_svm::system_program::ID,
        value: 7,
    }
    .into();

    // Pre-fund with just 1 lamport; payer must top up almost all rent.
    let result = svm.process_instruction(
        &ix,
        &[rich_signer_account(payer), prefunded_account(account, 1)],
    );
    assert!(
        result.is_ok(),
        "prefunded 1 lamport: {:?}",
        result.raw_result
    );

    let acc = result.account(&account).expect("account");
    assert_eq!(acc.data[0], 2, "discriminator");
    assert_eq!(acc.owner, quasar_test_misc::ID, "owner");
}
