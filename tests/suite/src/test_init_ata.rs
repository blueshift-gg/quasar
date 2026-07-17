use {
    crate::helpers::*,
    quasar_spl::get_associated_token_address_with_program_const,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::cpi::*,
};

// init with SPL Token.

#[test]
fn init_ata_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(ata_key),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init ATA should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_ata_spl_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, mint_key, wallet, 0, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::AccountAlreadyInitialized);
}

// init with Token-2022.

#[test]
fn init_ata_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaT22Instruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(ata_key),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init ATA (T22) should succeed: {:?}",
        result.raw_result
    );
}

#[test]
fn init_ata_t22_already_initialized() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaT22Instruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, mint_key, wallet, 0, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::AccountAlreadyInitialized);
}

// init_if_needed new ATA with SPL Token.

#[test]
fn init_if_needed_ata_spl_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(ata_key),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on new ATA should succeed: {:?}",
        result.raw_result
    );
}

// init_if_needed existing valid ATA with SPL Token.

#[test]
fn init_if_needed_ata_spl_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let existing = token_account(ata_key, mint_key, wallet, 100, token_program);
    let existing_data = existing.data.clone();
    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            existing,
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid ATA should succeed (no-op): {:?}",
        result.raw_result
    );
    // "No-op" must mean untouched: the existing account's bytes are
    // byte-identical after the idempotent init.
    let after = result.account(&ata_key).expect("existing account");
    assert_eq!(
        after.data, existing_data,
        "existing valid account must be left unmodified"
    );
}

// init_if_needed existing invalid ATA with SPL Token.

#[test]
fn init_if_needed_ata_spl_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, wrong_mint, wallet, 100, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn init_if_needed_ata_spl_existing_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_wallet = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, mint_key, wrong_wallet, 100, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn init_if_needed_ata_spl_existing_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            raw_account(
                ata_key,
                1_000_000,
                pack_token_data(mint_key, wallet, 100),
                Pubkey::default(),
            ),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    // The existing account is system-owned, so init takes the create
    // branch: SystemError::AccountAlreadyInUse.
    result.assert_error(quasar_svm::ProgramError::Custom(0));
}

// init_if_needed new ATA with Token-2022.

#[test]
fn init_if_needed_ata_t22_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        wallet,
        mint: mint_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(ata_key),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on new ATA should succeed (T22): {:?}",
        result.raw_result
    );
}

// init_if_needed existing valid ATA with Token-2022.

// init_if_needed existing invalid ATA with Token-2022.
