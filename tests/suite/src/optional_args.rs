use {
    crate::compat::{Instruction, Pubkey},
    crate::helpers::*,
    quasar_lang::client::{DynString, DynVec},
    quasar_test_misc::cpi::*,
};

// Happy-path tests use generated CPI structs

#[test]
fn option_u64_some_happy() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = OptionU64SomeInstruction {
        signer,
        value: Some(42),
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "Option<u64> Some(42): {:?}",
        result.raw_result
    );
}

#[test]
fn option_u64_none_happy() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = OptionU64NoneInstruction {
        signer,
        value: None,
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(result.is_ok(), "Option<u64> None: {:?}", result.raw_result);
}

#[test]
fn option_u64_some_wrong_value() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = OptionU64SomeInstruction {
        signer,
        value: Some(99),
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

#[test]
fn option_address_some_happy() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let addr = Pubkey::new_unique();
    let ix: Instruction = OptionAddressSomeInstruction {
        signer,
        addr: Some(addr),
    }
    .into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "Option<Address> Some: {:?}",
        result.raw_result
    );
}

#[test]
fn option_address_none_happy() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let ix: Instruction = OptionAddressNoneInstruction { signer, addr: None }.into();
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    assert!(
        result.is_ok(),
        "Option<Address> None: {:?}",
        result.raw_result
    );
}

// Adversarial test: manually craft instruction data with tag=2 (invalid)
#[test]
fn option_u64_tag_two_rejected() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    // Discriminator 52 (option_u64_some expects Some(42))
    // Wire format: [disc, tag, PodU64 le bytes]
    let mut data = vec![52u8]; // discriminator
    data.push(2); // tag = 2, invalid because only 0 and 1 are valid
    data.extend_from_slice(&42u64.to_le_bytes()); // PodU64(42)
    let ix = solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

// Adversarial test: tag=0xFF (invalid)
#[test]
fn option_u64_tag_0xff_rejected() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    let mut data = vec![52u8]; // discriminator
    data.push(0xFF); // tag = 0xFF (invalid)
    data.extend_from_slice(&42u64.to_le_bytes());
    let ix = solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

// Adversarial test: truncated instruction data (disc only, no Option payload)
#[test]
fn option_u64_truncated_data_fails() {
    let mut svm = svm_misc();
    let signer = Pubkey::new_unique();
    // Only send the discriminator byte, no Option<u64> payload
    let ix = solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data: vec![52u8], // just discriminator, missing 9 bytes of OptionZc<PodU64>
    };
    let result = svm.process_instruction(&ix, &[signer_account(signer)]);
    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

fn optional_dynamic_ix(data: Vec<u8>) -> solana_instruction::Instruction {
    solana_instruction::Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            quasar_test_misc::ID,
            false,
        )],
        data,
    }
}

#[test]
fn optional_dynamic_args_none_none_use_compact_tags_only() {
    let mut svm = svm_misc();
    let data = vec![61u8, 0, 0];
    let ix = optional_dynamic_ix(data);

    let result = svm.process_instruction(&ix, &[]);

    assert!(
        result.is_ok(),
        "Option<String>/Option<Vec> None/None should decode from compact option tags: {:?}",
        result.raw_result
    );
}

#[test]
fn optional_dynamic_args_some_some_use_compact_tail_payloads() {
    let mut svm = svm_misc();
    let mut data = vec![61u8, 1, 1];
    data.push(6);
    data.extend_from_slice(b"quasar");
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(quasar_test_misc::EXPECTED_ADDRESS.as_ref());
    let ix = optional_dynamic_ix(data);

    let result = svm.process_instruction(&ix, &[]);

    assert!(
        result.is_ok(),
        "Option<String>/Option<Vec> Some/Some should decode compact tagged tails: {:?}",
        result.raw_result
    );
}

#[test]
fn optional_dynamic_generated_client_uses_compact_header_then_tail_layout() {
    let ix: Instruction = OptionalDynamicArgInstruction {
        maybe_name: Some(DynString::<u8>::from("quasar")),
        maybe_addrs: Some(DynVec::<Pubkey, u16>::from(vec![
            quasar_test_misc::EXPECTED_ADDRESS,
        ])),
    }
    .into();

    let mut expected = vec![61u8, 1, 1];
    expected.push(6);
    expected.extend_from_slice(b"quasar");
    expected.extend_from_slice(&1u16.to_le_bytes());
    expected.extend_from_slice(quasar_test_misc::EXPECTED_ADDRESS.as_ref());

    assert_eq!(ix.data, expected);
}

#[test]
fn optional_dynamic_args_invalid_tag_fails() {
    let mut svm = svm_misc();
    let data = vec![61u8, 2, 0];
    let ix = optional_dynamic_ix(data);

    let result = svm.process_instruction(&ix, &[]);

    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

#[test]
fn optional_dynamic_args_some_missing_prefix_fails() {
    let mut svm = svm_misc();
    let data = vec![61u8, 1, 0];
    let ix = optional_dynamic_ix(data);

    let result = svm.process_instruction(&ix, &[]);

    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}

#[test]
fn optional_dynamic_args_some_truncated_payload_fails() {
    let mut svm = svm_misc();
    let mut data = vec![61u8, 1, 0];
    data.push(6);
    data.extend_from_slice(b"qua");
    let ix = optional_dynamic_ix(data);

    let result = svm.process_instruction(&ix, &[]);

    result.assert_error(crate::compat::ProgramError::InvalidInstructionData);
}
