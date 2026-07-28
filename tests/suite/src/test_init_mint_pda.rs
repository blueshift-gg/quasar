use {
    crate::compat::{Account, Instruction, Pubkey},
    crate::helpers::*,
    quasar_test_token_init::cpi::*,
};

// PDA init mint with SPL Token.

#[test]
fn init_mint_pda_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();

    let (mint_pda, _bump) =
        Pubkey::find_program_address(&[b"mint", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitMintPdaInstruction { payer }.into();

    let result = svm.process_instruction(
        &instruction,
        &[rich_signer_account(payer), empty_account(mint_pda)],
    );
    assert!(
        result.is_ok(),
        "init mint PDA should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_mint_pda_spl_wrong_address() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();

    let wrong_key = Pubkey::new_unique();

    let mut instruction: Instruction = InitMintPdaInstruction { payer }.into();
    // The client derives the canonical PDA meta; repoint it at the address
    // under test.
    instruction.accounts[1].pubkey = wrong_key;

    let result = svm.process_instruction(
        &instruction,
        &[rich_signer_account(payer), empty_account(wrong_key)],
    );
    result.assert_error(crate::compat::ProgramError::Custom(
        quasar_lang::prelude::QuasarError::InvalidPda as u32,
    ));
}

// PDA init mint with Token-2022.

#[test]
fn init_mint_pda_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();

    let (mint_pda, _bump) =
        Pubkey::find_program_address(&[b"mint", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitMintPdaT22Instruction { payer }.into();

    let result = svm.process_instruction(
        &instruction,
        &[rich_signer_account(payer), empty_account(mint_pda)],
    );
    assert!(
        result.is_ok(),
        "init mint PDA (T22) should succeed: {:?}",
        result.raw_result
    );
}

// Pre-funded PDA mint init with SPL Token.

#[test]
fn init_mint_pda_spl_prefunded_partial() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();

    let (mint_pda, _bump) =
        Pubkey::find_program_address(&[b"mint", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitMintPdaInstruction { payer }.into();

    let prefund = 500_000u64;
    let payer_lamports = 100_000_000_000u64;
    let result = svm.process_instruction(
        &instruction,
        &[
            Account {
                address: payer,
                lamports: payer_lamports,
                data: vec![],
                owner: crate::compat::system_program::ID,
                executable: false,
            },
            prefunded_account(mint_pda, prefund),
        ],
    );
    assert!(
        result.is_ok(),
        "prefunded partial mint PDA: {:?}",
        result.raw_result
    );

    // Payer only charged the delta
    let payer_after = result.account(&payer).expect("payer");
    let charged = payer_lamports - payer_after.lamports;
    assert!(charged > 0, "payer charged something");
    let acc = result.account(&mint_pda).expect("mint");
    assert!(charged < acc.lamports, "payer charged less than full rent");
}

#[test]
fn init_mint_pda_spl_prefunded_excess() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();

    let (mint_pda, _bump) =
        Pubkey::find_program_address(&[b"mint", payer.as_ref()], &quasar_test_token_init::ID);

    let payer_lamports = 100_000_000_000u64;
    let instruction: Instruction = InitMintPdaInstruction { payer }.into();

    let result = svm.process_instruction(
        &instruction,
        &[
            Account {
                address: payer,
                lamports: payer_lamports,
                data: vec![],
                owner: crate::compat::system_program::ID,
                executable: false,
            },
            prefunded_account(mint_pda, 100_000_000),
        ],
    );
    assert!(
        result.is_ok(),
        "prefunded excess mint PDA: {:?}",
        result.raw_result
    );

    // Payer not charged
    let payer_after = result.account(&payer).expect("payer");
    assert_eq!(payer_after.lamports, payer_lamports, "payer not charged");
}
