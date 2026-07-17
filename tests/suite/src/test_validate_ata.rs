use {
    crate::helpers::*,
    quasar_spl::get_associated_token_address_with_program_const,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_validate::cpi::*,
};

// Account<Token> with SPL Token, ValidateAtaCheck.

#[test]
fn ata_spl_happy() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn ata_spl_wrong_address() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let wrong_ata = Pubkey::new_unique();

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: wrong_ata,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(wrong_ata, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidSeeds);
}

#[test]
fn ata_spl_wrong_mint() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, wrong_mint, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn ata_spl_wrong_authority() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let wrong_wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wrong_wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn ata_spl_wrong_owner() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                ata_key,
                1_000_000,
                pack_token_data(mint_key, wallet, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(quasar_svm::ProgramError::Runtime("IllegalOwner".into()));
}

// Account<Token2022>, ValidateAta2022Check.

#[test]
fn ata_t22_happy() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAta2022CheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

// InterfaceAccount<Token> with SPL Token, ValidateAtaInterfaceCheck.

#[test]
fn ata_interface_spl_happy() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn ata_interface_spl_wrong_address() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let wrong_ata = Pubkey::new_unique();

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: wrong_ata,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(wrong_ata, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidSeeds);
}

#[test]
fn ata_interface_spl_wrong_mint() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, wrong_mint, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn ata_interface_spl_wrong_authority() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let wrong_wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wrong_wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn ata_interface_spl_wrong_owner() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            raw_account(
                ata_key,
                1_000_000,
                pack_token_data(mint_key, wallet, 100),
                Pubkey::default(),
            ),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(quasar_svm::ProgramError::Runtime("IllegalOwner".into()));
}

// InterfaceAccount<Token> with Token-2022, ValidateAtaInterfaceCheck.

#[test]
fn ata_interface_t22_happy() {
    let mut svm = svm_validate();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = ValidateAtaInterfaceCheckInstruction {
        ata: ata_key,
        mint: mint_key,
        wallet,
        token_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            token_account(ata_key, mint_key, wallet, 100, token_program),
            mint_account(mint_key, authority, 6, token_program),
            signer_account(wallet),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}
