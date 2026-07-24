use {
    crate::compat::{Account, Instruction, Pubkey},
    crate::helpers::*,
    quasar_test_token_init::cpi::*,
};

// PDA init token with SPL Token.

#[test]
fn init_token_pda_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    // Derive the PDA: seeds = [b"token", payer]
    let (token_pda, _bump) =
        Pubkey::find_program_address(&[b"token", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitTokenPdaInstruction {
        payer,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(token_pda),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init token PDA should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_token_pda_spl_wrong_address() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let wrong_key = Pubkey::new_unique();

    let mut instruction: Instruction = InitTokenPdaInstruction {
        payer,
        mint: mint_key,
    }
    .into();
    // The client derives the canonical PDA meta; repoint it at the address
    // under test.
    instruction.accounts[1].pubkey = wrong_key;

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(wrong_key),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    result.assert_error(crate::compat::ProgramError::Custom(
        quasar_lang::prelude::QuasarError::InvalidPda as u32,
    ));
}

// PDA init token with Token-2022.

#[test]
fn init_token_pda_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();

    let (token_pda, _bump) =
        Pubkey::find_program_address(&[b"token", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitTokenPdaT22Instruction {
        payer,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(token_pda),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init token PDA (T22) should succeed: {:?}",
        result.raw_result
    );
}

// Pre-funded PDA token init with SPL Token.

#[test]
fn init_token_pda_spl_prefunded_partial() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let (token_pda, _bump) =
        Pubkey::find_program_address(&[b"token", payer.as_ref()], &quasar_test_token_init::ID);

    let instruction: Instruction = InitTokenPdaInstruction {
        payer,
        mint: mint_key,
    }
    .into();

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
            prefunded_account(token_pda, prefund),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "prefunded partial token PDA: {:?}",
        result.raw_result
    );

    // Payer only charged the delta
    let payer_after = result.account(&payer).expect("payer");
    let charged = payer_lamports - payer_after.lamports;
    assert!(charged > 0, "payer charged something");
    let acc = result.account(&token_pda).expect("token");
    assert!(charged < acc.lamports, "payer charged less than full rent");
}

#[test]
fn init_token_pda_spl_prefunded_excess() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let (token_pda, _bump) =
        Pubkey::find_program_address(&[b"token", payer.as_ref()], &quasar_test_token_init::ID);

    let payer_lamports = 100_000_000_000u64;
    let instruction: Instruction = InitTokenPdaInstruction {
        payer,
        mint: mint_key,
    }
    .into();

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
            prefunded_account(token_pda, 100_000_000),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "prefunded excess token PDA: {:?}",
        result.raw_result
    );

    // Payer not charged (pre-fund covers rent)
    let payer_after = result.account(&payer).expect("payer");
    assert_eq!(payer_after.lamports, payer_lamports, "payer not charged");
}
