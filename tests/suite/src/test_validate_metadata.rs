use {
    crate::helpers::*,
    quasar_svm::{Instruction, Pubkey},
    quasar_test_metadata_validate::cpi::*,
    solana_address::Address,
};

const METADATA_PROGRAM_BYTES: [u8; 32] = [
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205, 88, 184, 108, 115,
    26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
];

fn derive_metadata_pda(mint: &Pubkey) -> Pubkey {
    let program = Address::new_from_array(METADATA_PROGRAM_BYTES);
    let (addr, _) =
        Address::find_program_address(&[b"metadata", program.as_ref(), mint.as_ref()], &program);
    Pubkey::from(addr.to_bytes())
}

fn rent_sysvar() -> Pubkey {
    Pubkey::from(Address::from_str_const("SysvarRent111111111111111111111111111111111").to_bytes())
}

fn derive_master_edition_pda(mint: &Pubkey) -> Pubkey {
    let program = Address::new_from_array(METADATA_PROGRAM_BYTES);
    let (addr, _) = Address::find_program_address(
        &[b"metadata", program.as_ref(), mint.as_ref(), b"edition"],
        &program,
    );
    Pubkey::from(addr.to_bytes())
}

// Bare Account<MetadataAccount>, ValidateBareMetadata disc=3.

#[test]
fn bare_metadata_happy() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();
    let meta_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMetadataInstruction { metadata: meta_key }.into();

    let result = svm.process_instruction(&instruction, &[metadata_account(meta_key, ua, mint)]);
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn bare_metadata_wrong_owner() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();
    let meta_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMetadataInstruction { metadata: meta_key }.into();

    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            meta_key,
            1_000_000,
            build_metadata_account_data(ua, mint),
            Pubkey::default(),
        )],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(quasar_svm::ProgramError::Runtime("IllegalOwner".into()));
}

#[test]
fn bare_metadata_wrong_key_byte() {
    let mut svm = svm_metadata_validate();
    let meta_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMetadataInstruction { metadata: meta_key }.into();

    let mut data = vec![0u8; 128];
    data[0] = 6; // master edition key, not metadata
    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            meta_key,
            1_000_000,
            data,
            metadata_program_id(),
        )],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn bare_metadata_data_too_small() {
    let mut svm = svm_metadata_validate();
    let meta_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMetadataInstruction { metadata: meta_key }.into();

    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            meta_key,
            1_000_000,
            vec![4u8; 10],
            metadata_program_id(),
        )],
    );
    result.assert_error(quasar_svm::ProgramError::AccountDataTooSmall);
}

#[test]
fn bare_metadata_all_zeros() {
    let mut svm = svm_metadata_validate();
    let meta_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMetadataInstruction { metadata: meta_key }.into();

    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            meta_key,
            1_000_000,
            vec![0u8; 128],
            metadata_program_id(),
        )],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

// Bare Account<MasterEditionAccount>, ValidateBareMasterEdition disc=4.

#[test]
fn bare_master_edition_happy() {
    let mut svm = svm_metadata_validate();
    let me_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMasterEditionInstruction {
        master_edition: me_key,
    }
    .into();

    let result =
        svm.process_instruction(&instruction, &[master_edition_account(me_key, 0, Some(0))]);
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn bare_master_edition_unlimited() {
    let mut svm = svm_metadata_validate();
    let me_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMasterEditionInstruction {
        master_edition: me_key,
    }
    .into();

    let result = svm.process_instruction(&instruction, &[master_edition_account(me_key, 42, None)]);
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn bare_master_edition_wrong_owner() {
    let mut svm = svm_metadata_validate();
    let me_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMasterEditionInstruction {
        master_edition: me_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            me_key,
            1_000_000,
            build_master_edition_data(0, Some(0)),
            Pubkey::default(),
        )],
    );
    // the harness maps InstructionErrors without a dedicated variant to their Debug
    // string
    result.assert_error(quasar_svm::ProgramError::Runtime("IllegalOwner".into()));
}

#[test]
fn bare_master_edition_wrong_key() {
    let mut svm = svm_metadata_validate();
    let me_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMasterEditionInstruction {
        master_edition: me_key,
    }
    .into();

    let mut data = vec![0u8; 32];
    data[0] = 4; // metadata key, not master edition
    let result = svm.process_instruction(
        &instruction,
        &[raw_account(me_key, 1_000_000, data, metadata_program_id())],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn bare_master_edition_data_too_small() {
    let mut svm = svm_metadata_validate();
    let me_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateBareMasterEditionInstruction {
        master_edition: me_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[raw_account(
            me_key,
            1_000_000,
            vec![6u8; 10],
            metadata_program_id(),
        )],
    );
    result.assert_error(quasar_svm::ProgramError::AccountDataTooSmall);
}

// Metadata with behavior, ValidateMetadataCheck disc=0.

#[test]
fn metadata_check_happy() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();

    let meta_key = derive_metadata_pda(&mint);

    let instruction: Instruction = ValidateMetadataCheckInstruction {
        metadata_program: metadata_program_id(),
        mint,
        metadata: meta_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            metadata_account(meta_key, ua, mint),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn metadata_check_wrong_mint_in_data() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();

    let meta_key = derive_metadata_pda(&mint);

    let instruction: Instruction = ValidateMetadataCheckInstruction {
        metadata_program: metadata_program_id(),
        mint,
        metadata: meta_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            metadata_account(meta_key, ua, wrong_mint), // data has wrong mint
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

#[test]
fn metadata_check_wrong_pda() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();
    let wrong_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateMetadataCheckInstruction {
        metadata_program: metadata_program_id(),
        mint,
        metadata: wrong_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            metadata_account(wrong_key, ua, mint),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidSeeds);
}

// Metadata with update_authority, ValidateMetadataWithUa disc=1.

#[test]
fn metadata_with_ua_happy() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();

    let meta_key = derive_metadata_pda(&mint);

    let instruction: Instruction = ValidateMetadataWithUaInstruction {
        metadata_program: metadata_program_id(),
        mint,
        update_authority: ua,
        metadata: meta_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            signer_account(ua),
            metadata_account(meta_key, ua, mint),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn metadata_with_ua_wrong_authority() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let ua = Pubkey::new_unique();
    let wrong_ua = Pubkey::new_unique();

    let meta_key = derive_metadata_pda(&mint);

    let instruction: Instruction = ValidateMetadataWithUaInstruction {
        metadata_program: metadata_program_id(),
        mint,
        update_authority: ua,
        metadata: meta_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            signer_account(ua),
            metadata_account(meta_key, wrong_ua, mint), // data has wrong_ua
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidAccountData);
}

// Master Edition with behavior, ValidateMasterEditionCheck disc=2.

#[test]
fn master_edition_check_happy() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();

    let me_key = derive_master_edition_pda(&mint);

    let instruction: Instruction = ValidateMasterEditionCheckInstruction {
        metadata_program: metadata_program_id(),
        mint,
        master_edition: me_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            master_edition_account(me_key, 0, Some(0)),
        ],
    );
    assert!(result.is_ok(), "should succeed: {:?}", result.raw_result);
}

#[test]
fn master_edition_check_wrong_pda() {
    let mut svm = svm_metadata_validate();
    let mint = Pubkey::new_unique();
    let wrong_key = Pubkey::new_unique();

    let instruction: Instruction = ValidateMasterEditionCheckInstruction {
        metadata_program: metadata_program_id(),
        mint,
        master_edition: wrong_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            metadata_program_account(),
            signer_account(mint),
            master_edition_account(wrong_key, 0, Some(0)),
        ],
    );
    result.assert_error(quasar_svm::ProgramError::InvalidSeeds);
}

// Init Metadata via CPI, InitMetadataTest disc=10.
// Full end-to-end: create mint, CPI to Metaplex, then verify prefix fields.

#[test]
fn init_metadata_happy() {
    let mut svm = svm_metadata_validate();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let update_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let meta_key = derive_metadata_pda(&mint_key);

    let instruction: Instruction = InitMetadataTestInstruction {
        payer,
        metadata_program: metadata_program_id(),
        system_program: quasar_svm::system_program::ID,
        rent: rent_sysvar(),
        mint: mint_key,
        mint_authority,
        update_authority,
        metadata: meta_key,
    }
    .into();

    // All accounts the instruction + CPI need. SVM matches by address, not order.
    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            empty_account(meta_key),
            mint_account(mint_key, mint_authority, 0, token_program),
            signer_account(mint_authority),
            signer_account(update_authority),
        ],
    );
    assert!(
        result.is_ok(),
        "init metadata should succeed: {:?}",
        result.raw_result
    );
}

// Init Master Edition via CPI, InitMasterEditionTest disc=11.

#[test]
fn init_master_edition_happy() {
    let mut svm = svm_metadata_validate();
    let payer = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let mint_authority = Pubkey::new_unique();
    let update_authority = Pubkey::new_unique();
    let token_program = spl_token_program_id();

    let meta_key = derive_metadata_pda(&mint_key);
    let me_key = derive_master_edition_pda(&mint_key);

    // Both metadata_account and master_edition are init'd by derive behaviors.
    let instruction: Instruction = InitMasterEditionTestInstruction {
        payer,
        metadata_program: metadata_program_id(),
        system_program: quasar_svm::system_program::ID,
        rent: rent_sysvar(),
        mint: mint_key,
        update_authority,
        mint_authority,
        token_program,
        metadata_account: meta_key,
        master_edition: me_key,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            rich_signer_account(payer),
            nft_mint_account(mint_key, mint_authority, token_program),
            signer_account(update_authority),
            signer_account(mint_authority),
            empty_account(meta_key),
            empty_account(me_key),
        ],
    );
    assert!(
        result.is_ok(),
        "init master edition should succeed: {:?}\nlogs: {:?}",
        result.raw_result,
        result.logs,
    );
}
