use {
    crate::helpers::*,
    quasar_spl::get_associated_token_address_with_program_const,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_token_init::client::*,
};

// ===========================================================================
// init — SPL Token
// ===========================================================================

#[test]
fn init_ata_spl_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init on already-initialized ATA should fail"
    );
}

// ===========================================================================
// init — Token-2022
// ===========================================================================

#[test]
fn init_ata_t22_happy() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init on already-initialized ATA should fail (T22)"
    );
}

// ===========================================================================
// init_if_needed new — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_ata_spl_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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

// ===========================================================================
// init_if_needed existing valid — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_ata_spl_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, mint_key, wallet, 100, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid ATA should succeed (no-op): {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing bad — SPL Token
// ===========================================================================

#[test]
fn init_if_needed_ata_spl_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong mint should fail"
    );
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
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong authority should fail"
    );
}

#[test]
fn init_if_needed_ata_spl_existing_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            raw_account(ata_key, 1_000_000, pack_token_data(mint_key, wallet, 100), Pubkey::default()),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong account owner should fail"
    );
}

// ===========================================================================
// init_if_needed new — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_ata_t22_happy_new() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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

// ===========================================================================
// init_if_needed existing valid — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_ata_t22_existing_valid() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            token_account(ata_key, mint_key, wallet, 100, token_program),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_ok(),
        "init_if_needed on existing valid ATA should succeed (T22, no-op): {:?}",
        result.raw_result
    );
}

// ===========================================================================
// init_if_needed existing bad — Token-2022
// ===========================================================================

#[test]
fn init_if_needed_ata_t22_existing_wrong_mint() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong mint should fail (T22)"
    );
}

#[test]
fn init_if_needed_ata_t22_existing_wrong_authority() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let wrong_wallet = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
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
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong authority should fail (T22)"
    );
}

#[test]
fn init_if_needed_ata_t22_existing_wrong_owner() {
    let mut svm = svm_init();
    let payer = Pubkey::new_unique();
    let wallet = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let token_program = token_2022_program_id();
    let ata_program = ata_program_id();
    let (ata_key, _) =
        get_associated_token_address_with_program_const(&wallet, &mint_key, &token_program);

    let instruction: Instruction = InitIfNeededAtaT22Instruction {
        payer,
        ata: ata_key,
        wallet,
        mint: mint_key,
        token_program,
        system_program: quasar_svm::system_program::ID,
        ata_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            raw_account(ata_key, 1_000_000, pack_token_data(mint_key, wallet, 100), Pubkey::default()),
            signer_account(wallet),
            mint_account(mint_key, mint_authority, 6, token_program),
        ],
    );
    assert!(
        result.is_err(),
        "init_if_needed ATA with wrong account owner should fail (T22)"
    );
}
