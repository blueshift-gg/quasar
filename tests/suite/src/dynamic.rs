use {
    crate::helpers::mollusk_for_program,
    mollusk_svm::{program::keyed_account_for_system_program, result::ProgramResult, Mollusk},
    quasar_lang::prelude::ProgramError,
    quasar_test_misc::cpi::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
};

const DYNAMIC_ACCOUNT_DISC: u8 = 5;
const DYNAMIC_HEADER_SIZE: usize = 1; // disc byte only (compact header follows)
const MIXED_ACCOUNT_DISC: u8 = 6;
const MIXED_FIXED_SIZE: usize = 32 + 8; // Address + u64
const SMALL_PREFIX_DISC: u8 = 7;
const DYN_STR_DISC: u8 = 8;
const DYN_BYTES_DISC: u8 = 9;
const DYN_FIXED_SIZE: usize = 32; // Address

fn build_dynamic_account_data(name: &[u8], tags: &[Address]) -> Vec<u8> {
    // Compact layout: [disc(1)][header: u8 name_len(1) + u16 tags_count(2)][tail:
    // name_bytes + tag_bytes]
    let name_len = name.len();
    let tags_count = tags.len();
    let tags_bytes = tags_count * 32;
    let header_size = 1 + 2; // u8 name_len + u16 tags_count
    let total = DYNAMIC_HEADER_SIZE + header_size + name_len + tags_bytes;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = DYNAMIC_ACCOUNT_DISC;
    offset += 1;

    // Header: all length prefixes grouped together
    data[offset] = name_len as u8;
    offset += 1;
    data[offset..offset + 2].copy_from_slice(&(tags_count as u16).to_le_bytes());
    offset += 2;

    // Tail: all data grouped together
    data[offset..offset + name_len].copy_from_slice(name);
    offset += name_len;
    for (i, tag) in tags.iter().enumerate() {
        data[offset + i * 32..offset + (i + 1) * 32].copy_from_slice(tag.as_ref());
    }

    data
}

fn build_mixed_account_data(authority: Address, value: u64, label: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][value(8)][u8:label_len(1)][label_bytes]
    let label_len = label.len();
    let total = 1 + MIXED_FIXED_SIZE + 1 + label_len;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = MIXED_ACCOUNT_DISC;
    offset += 1;

    data[offset..offset + 32].copy_from_slice(authority.as_ref());
    offset += 32;

    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    offset += 8;

    data[offset] = label_len as u8;
    offset += 1;

    data[offset..offset + label_len].copy_from_slice(label);

    data
}

fn build_small_prefix_account_data(tag: &[u8], scores: &[u8]) -> Vec<u8> {
    // Compact layout: [disc(1)][header: u8 tag_len(1) + u16 scores_count(2)][tail:
    // tag_bytes + score_bytes]
    let tag_len = tag.len();
    let scores_count = scores.len();
    let header_size = 1 + 2; // u8 tag_len + u16 scores_count
    let total = 1 + header_size + tag_len + scores_count;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = SMALL_PREFIX_DISC;
    offset += 1;

    // Header: all length prefixes grouped together
    data[offset] = tag_len as u8;
    offset += 1;
    data[offset..offset + 2].copy_from_slice(&(scores_count as u16).to_le_bytes());
    offset += 2;

    // Tail: all data grouped together
    data[offset..offset + tag_len].copy_from_slice(tag);
    offset += tag_len;
    data[offset..offset + scores_count].copy_from_slice(scores);

    data
}

fn build_readback_instruction(
    account: Address,
    expected_name_len: u8,
    expected_tags_count: u8,
) -> Instruction {
    // Instruction data: [disc(24)][expected_name_len(u8)][expected_tags_count(u8)]
    let data = vec![24, expected_name_len, expected_tags_count];
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data,
    }
}

fn build_mutate_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    new_name: &[u8],
) -> Instruction {
    // Instruction data: [disc(26)][u8:name_len][name_bytes]
    // String<N> instruction args use u8 prefix on the wire.
    let mut data = vec![26];
    data.push(new_name.len() as u8);
    data.extend_from_slice(new_name);
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            solana_instruction::AccountMeta::new(account, false),
            solana_instruction::AccountMeta::new(payer, true),
            solana_instruction::AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }
}

fn build_mutate_then_readback_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    expected_tags_count: u8,
    new_name: &[u8],
) -> Instruction {
    // Layout: [disc(27)][expected_tags_count(u8)][u8:name_len][name_bytes]
    // Fixed args occupy the instruction header; dynamic args form a suffix.
    let mut data = vec![27];
    data.push(expected_tags_count);
    data.push(new_name.len() as u8);
    data.extend_from_slice(new_name);
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            solana_instruction::AccountMeta::new(account, false),
            solana_instruction::AccountMeta::new(payer, true),
            solana_instruction::AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }
}

fn build_dynamic_view_mut_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    new_name: &[u8],
    new_tags: &[Address],
) -> Instruction {
    // Compact layout: [disc(58)][header: u8 name_len, u16 tags_count][tail:
    // name_bytes, tag_bytes]
    let mut data = vec![58];
    // Header: all length prefixes grouped together
    data.push(new_name.len() as u8);
    data.extend_from_slice(&(new_tags.len() as u16).to_le_bytes());
    // Tail: all data grouped together
    data.extend_from_slice(new_name);
    for tag in new_tags {
        data.extend_from_slice(tag.as_ref());
    }
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            solana_instruction::AccountMeta::new(account, false),
            solana_instruction::AccountMeta::new(payer, true),
            solana_instruction::AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }
}

fn build_dynamic_view_mut_missing_field_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    new_name: &[u8],
) -> Instruction {
    let mut data = vec![59];
    data.push(new_name.len() as u8);
    data.extend_from_slice(new_name);
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            solana_instruction::AccountMeta::new(account, false),
            solana_instruction::AccountMeta::new(payer, true),
            solana_instruction::AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }
}

fn build_dyn_str_account_data(authority: Address, label: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][u8_len(1)][label_bytes...]
    assert!(label.len() <= 255, "label exceeds String<255> capacity");
    let mut data = vec![0u8; 1 + DYN_FIXED_SIZE + 1 + label.len()];
    data[0] = DYN_STR_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33] = label.len() as u8;
    data[34..].copy_from_slice(label);
    data
}

fn build_dyn_bytes_account_data(authority: Address, payload: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][u16_count_le(2)][data_bytes...]
    assert!(
        payload.len() <= 1024,
        "payload exceeds Vec<u8, 1024> capacity"
    );
    let count = payload.len() as u16;
    let mut data = vec![0u8; 1 + DYN_FIXED_SIZE + 2 + payload.len()];
    data[0] = DYN_BYTES_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..35].copy_from_slice(&count.to_le_bytes());
    data[35..].copy_from_slice(payload);
    data
}

fn build_dyn_str_check_instruction(account: Address, expected_len: u8) -> Instruction {
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![28, expected_len],
    }
}

fn build_dyn_bytes_check_instruction(account: Address, expected_len: u8) -> Instruction {
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![29, expected_len],
    }
}

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_misc::ID, "quasar_test_misc")
}

// ============================================================================
// Dynamic Account — Basic Validation
// ============================================================================

#[test]
fn test_dynamic_instruction_invalid_utf8_rejected() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = DynamicInstructionCheckInstruction {
        authority,
        name: quasar_lang::client::DynString::from(vec![0xFF]),
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(authority, authority_account)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidInstructionData)
    );
}

#[test]
fn test_dynamic_account_name_exceeds_max_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Build valid account, then corrupt: set name length prefix > max (8)
    let mut data = build_dynamic_account_data(b"hi", &[]);

    // Corrupt name length prefix (at offset 1) to exceed max of 8
    data[1] = 100;

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "name length exceeding max must be rejected"
    );
}

#[test]
fn test_dynamic_account_truncated_data_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Build valid account with name="hello" (5 bytes), then truncate data
    // so the name prefix declares more bytes than available
    let mut data = build_dynamic_account_data(b"hello", &[]);

    // Truncate: keep disc + name prefix but remove the name data
    data.truncate(DYNAMIC_HEADER_SIZE + 1); // just disc + u8 prefix, no bytes

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "truncated data must be rejected"
    );
}

#[test]
fn test_dynamic_account_valid_data_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag = Address::new_unique();
    let data = build_dynamic_account_data(b"hello", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid dynamic account data should be accepted: {:?}",
        result.program_result
    );
}

// ============================================================================
// Dynamic Account — Edge Cases
// ============================================================================

#[test]
fn test_dynamic_account_empty_string_and_empty_vec() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(b"", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty string + empty vec should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_account_string_at_exact_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(b"12345678", &[]); // exactly 8 bytes = max
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "string at exact max (8 bytes) should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_account_string_exceeds_max_by_one() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(b"123456789", &[]); // 9 bytes > max of 8
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "string at max+1 (9 bytes) must be rejected"
    );
}

#[test]
fn test_dynamic_account_vec_at_exact_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let data = build_dynamic_account_data(b"hi", &[tag1, tag2]); // exactly 2 = max
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "vec at exact max (2 tags) should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_account_vec_exceeds_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tags: Vec<Address> = (0..3).map(|_| Address::new_unique()).collect();
    let data = build_dynamic_account_data(b"hi", &tags); // 3 > max of 2
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "vec exceeding max (3 tags) must be rejected"
    );
}

#[test]
fn test_dynamic_account_trailing_bytes_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();

    let mut data = build_dynamic_account_data(b"hi", &[]);
    data.extend_from_slice(&[0u8; 64]); // extra trailing bytes (slack space)
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "account with trailing bytes (slack space) should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_account_wrong_discriminator() {
    let mollusk = setup();
    let account = Address::new_unique();

    let mut data = build_dynamic_account_data(b"hi", &[]);
    data[0] = 99; // wrong discriminator
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData)
    );
}

#[test]
fn test_dynamic_account_minimum_size_empty_fields() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Minimum valid data: disc(1) + u8 name_len=0(1) + u16 tags_count=0(2) = 4
    // bytes
    let data = build_dynamic_account_data(b"", &[]);
    assert_eq!(
        data.len(),
        4,
        "minimum data size for DynamicAccount with empty fields"
    );
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "minimum-size account should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_account_too_small_for_prefixes() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Only disc byte — not enough for the u32 name prefix
    let data = vec![DYNAMIC_ACCOUNT_DISC];
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "data too small for prefix bytes must be rejected"
    );
}

// ============================================================================
// MixedAccount (fixed + dynamic fields, discriminator = 6)
// ============================================================================

#[test]
fn test_mixed_account_valid_data() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_mixed_account_data(authority, 42, b"test label");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid mixed account should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_mixed_account_empty_label() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_mixed_account_data(authority, 0, b"");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "mixed account with empty label should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_mixed_account_label_at_max() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let label = [b'x'; 32]; // exactly 32 = max
    let data = build_mixed_account_data(authority, 99, &label);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "mixed account label at exact max should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_mixed_account_label_exceeds_max() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let label = [b'x'; 33]; // 33 > max of 32
    let data = build_mixed_account_data(authority, 0, &label);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "mixed account label exceeding max must be rejected"
    );
}

#[test]
fn test_mixed_account_wrong_discriminator() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let mut data = build_mixed_account_data(authority, 42, b"hi");
    data[0] = 99; // wrong disc
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData)
    );
}

#[test]
fn test_mixed_account_truncated_in_fixed_section() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Only disc + partial authority (20 bytes instead of 32)
    let data = vec![MIXED_ACCOUNT_DISC; 21];
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "data truncated in fixed section must be rejected"
    );
}

#[test]
fn test_mixed_account_truncated_in_dynamic_section() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let mut data = build_mixed_account_data(authority, 42, b"hello");
    // Corrupt: set label prefix to claim 100 bytes but data only has 5
    let label_offset = 1 + MIXED_FIXED_SIZE;
    data[label_offset] = 100;
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MixedAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "data truncated in dynamic section must be rejected"
    );
}

// ============================================================================
// SmallPrefixAccount (u8 prefix, discriminator = 7)
// ============================================================================

#[test]
fn test_small_prefix_valid_data() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_small_prefix_account_data(b"hello", &[10, 20, 30]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid small prefix account should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_small_prefix_empty_fields() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_small_prefix_account_data(b"", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty small prefix account should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_small_prefix_tag_at_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag = [b'a'; 100]; // exactly 100 = max
    let data = build_small_prefix_account_data(&tag, &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "tag at exact max should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_small_prefix_tag_exceeds_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag = [b'a'; 101]; // 101 > max of 100
    let data = build_small_prefix_account_data(&tag, &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "tag exceeding max must be rejected"
    );
}

#[test]
fn test_small_prefix_scores_at_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let scores: Vec<u8> = (0..10).collect(); // exactly 10 = max
    let data = build_small_prefix_account_data(b"x", &scores);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "scores at exact max should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_small_prefix_scores_exceeds_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let scores: Vec<u8> = (0..11).collect(); // 11 > max of 10
    let data = build_small_prefix_account_data(b"x", &scores);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "scores exceeding max must be rejected"
    );
}

#[test]
fn test_small_prefix_truncated_data() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + tag prefix says 50 bytes but only provide 3
    let data = vec![SMALL_PREFIX_DISC, 50, b'a', b'b', b'c'];
    let _ = data.len();
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SmallPrefixCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "truncated small prefix data must be rejected"
    );
}

// ============================================================================
// Dynamic Accessor Readback (discriminator = 24)
// ============================================================================

#[test]
fn test_dynamic_readback_correct_lengths() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"hello", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_readback_instruction(account, 5, 1);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "readback with correct lengths should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_readback_empty_fields() {
    let mollusk = setup();
    let account = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_readback_instruction(account, 0, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "readback with empty fields should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_readback_max_fields() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"12345678", &[tag1, tag2]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_readback_instruction(account, 8, 2);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "readback with max fields should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_dynamic_readback_wrong_name_len() {
    let mollusk = setup();
    let account = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"hello", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_readback_instruction(account, 3, 0); // 3 != 5
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(1)),
        "wrong name length should return Custom(1)"
    );
}

#[test]
fn test_dynamic_readback_wrong_tags_count() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"hi", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_readback_instruction(account, 2, 0); // 0 != 1
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(2)),
        "wrong tags count should return Custom(2)"
    );
}

// ============================================================================
// Dynamic Mutation (discriminator = 26)
// ============================================================================

#[test]
fn test_dynamic_mutate_same_length_name() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"hello", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = build_mutate_instruction(account, payer, system_program, b"world");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "same-length mutation should succeed: {:?}",
        result.program_result
    );

    // Verify the account data was updated (compact layout: header then tail)
    let result_data = &result.resulting_accounts[0].1.data;
    assert_eq!(result_data[0], DYNAMIC_ACCOUNT_DISC);
    // Compact header: [name_len(1)][tags_count(2)]
    let name_len = result_data[1] as usize;
    assert_eq!(name_len, 5);
    let tags_count = u16::from_le_bytes(result_data[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&result_data[4..9], b"world");
    assert_eq!(&result_data[9..41], tag.as_ref());
}

#[test]
fn test_dynamic_mutate_shorter_name() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"hello", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = build_mutate_instruction(account, payer, system_program, b"hi");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "shorter name mutation should succeed: {:?}",
        result.program_result
    );

    let result_data = &result.resulting_accounts[0].1.data;
    let name_len = result_data[1] as usize;
    assert_eq!(name_len, 2);
    // Compact tail starts after header (disc + name_len + tags_count = 4)
    assert_eq!(&result_data[4..6], b"hi");
}

#[test]
fn test_dynamic_mutate_longer_name() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"hi", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = build_mutate_instruction(account, payer, system_program, b"12345678");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "longer name mutation (with realloc) should succeed: {:?}",
        result.program_result
    );

    let result_data = &result.resulting_accounts[0].1.data;
    let name_len = result_data[1] as usize;
    assert_eq!(name_len, 8);
    // Compact tail starts after header (disc + name_len + tags_count = 4)
    assert_eq!(&result_data[4..12], b"12345678");
}

#[test]
fn test_dynamic_mutate_to_empty() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"hello", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = build_mutate_instruction(account, payer, system_program, b"");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "mutation to empty string should succeed: {:?}",
        result.program_result
    );

    let result_data = &result.resulting_accounts[0].1.data;
    let name_len = result_data[1] as usize;
    assert_eq!(name_len, 0);
}

#[test]
fn test_dynamic_mutate_preserves_trailing_vec() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"abc", &[tag1, tag2]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Change name from "abc" (3) to "abcdef" (6) — grows, shifts tags
    let instruction = build_mutate_instruction(account, payer, system_program, b"abcdef");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "mutation with trailing vec should succeed: {:?}",
        result.program_result
    );

    let result_data = &result.resulting_accounts[0].1.data;
    // Compact header: [name_len(1)][tags_count(2)]
    let name_len = result_data[1] as usize;
    assert_eq!(name_len, 6);
    let tags_count = u16::from_le_bytes(result_data[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&result_data[4..10], b"abcdef");
    assert_eq!(&result_data[10..42], tag1.as_ref());
    assert_eq!(&result_data[42..74], tag2.as_ref());
}

#[test]
fn test_dynamic_mutate_exceeds_max_rejected() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"hi", &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Try to set name to 9 bytes (max is 8)
    let instruction = build_mutate_instruction(account, payer, system_program, b"123456789");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "mutation exceeding max must be rejected"
    );
}

// ============================================================================
// ADVERSARIAL TESTS: Crafted Prefix Attacks
// ============================================================================

/// u8 prefix claiming 255 bytes — validation must reject (max=8), not
/// wrap/panic
#[test]
fn test_adversarial_prefix_u8_max_name_len() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + u8 name prefix(255) + u16 tags prefix(0)
    let mut data = vec![0u8; 1 + 1 + 2];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 255; // name len = 255 (max=8)
    data[2..4].copy_from_slice(&0u16.to_le_bytes()); // tags count = 0

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "u8 name prefix=255 must be rejected (exceeds max=8)"
    );
}

/// u8 prefix just above max (9 when max=8) — off-by-one test
#[test]
fn test_adversarial_prefix_one_past_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Compact: disc + header(u8 prefix=9, u16 tags=0) + tail("aaaaaaaaa")
    let mut data = vec![0u8; 1 + 1 + 2 + 9];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 9; // name len = 9 (max=8)
    data[2..4].copy_from_slice(&0u16.to_le_bytes());
    data[4..13].copy_from_slice(b"aaaaaaaaa");

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "name len=9 (max=8) must be rejected even if data is valid UTF-8"
    );
}

/// Vec prefix claiming u16::MAX element count — tests count*elem_size overflow
/// path
#[test]
fn test_adversarial_vec_count_u32_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    // DynamicAccount: name="" (u8=0), tags count=u16::MAX
    let mut data = vec![0u8; 1 + 1 + 2]; // disc + u8 name prefix(0) + u16 tags prefix
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 0; // empty name
    data[2..4].copy_from_slice(&u16::MAX.to_le_bytes()); // 65535 tags

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "u16::MAX vec count must be rejected (exceeds max=2)"
    );
}

/// Vec prefix = 3 (max=2) — off-by-one on vec count
#[test]
fn test_adversarial_vec_count_one_past_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let tag3 = Address::new_unique();

    // Build with 3 tags (max=2): name="" u8=0, tags u16=3
    let mut data = vec![0u8; 1 + 1 + 2 + 32 * 3];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 0; // empty name
    data[2..4].copy_from_slice(&3u16.to_le_bytes()); // 3 tags (max=2)
    data[4..36].copy_from_slice(tag1.as_ref());
    data[36..68].copy_from_slice(tag2.as_ref());
    data[68..100].copy_from_slice(tag3.as_ref());

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "vec count=3 (max=2) must be rejected"
    );
}

/// Name prefix claims more tail bytes than available.
/// Compact header: [name_len=8][tags_count=0] but only 3 bytes of tail present.
#[test]
fn test_adversarial_name_data_overlaps_tags_prefix_region() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Compact: disc(1) + header(3) + 3 bytes of tail = 7 bytes total.
    // Header claims name_len=8, but only 3 tail bytes exist.
    let mut data = vec![0u8; 7];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 8; // u8 prefix claims 8 bytes
    data[2..4].copy_from_slice(&0u16.to_le_bytes()); // tags_count = 0
    data[4..7].copy_from_slice(b"abc"); // only 3 bytes of tail present

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "prefix claiming more bytes than available must fail"
    );
}

/// Tags prefix positioned correctly but data truncated: count=1 but only 16
/// of 32 tag bytes present.
#[test]
fn test_adversarial_vec_data_truncated_mid_element() {
    let mollusk = setup();
    let account = Address::new_unique();

    // name="" (u8=0) + tags count=1 but only 16 bytes (Address is 32)
    let mut data = vec![0u8; 1 + 1 + 2 + 16];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1] = 0; // empty name
    data[2..4].copy_from_slice(&1u16.to_le_bytes()); // 1 tag
                                                     // data[4..20] = 16 zero bytes (need 32 for Address)

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "truncated vec element data must fail"
    );
}

// ============================================================================
// ADVERSARIAL TESTS: Multi-byte UTF-8 Edge Cases
// ============================================================================

/// Valid 2-byte UTF-8 at field boundary: name = "é" (C3 A9) = 2 bytes
/// Ensures multi-byte chars at exact max boundary work (2 < max=8)
#[test]
fn test_adversarial_utf8_valid_multibyte_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(&[0xC3, 0xA9], &[]); // "é"
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid multi-byte UTF-8 must be accepted: {:?}",
        result.program_result
    );
}

/// Valid 4-byte UTF-8 emoji filling max (8 bytes = 2 emoji chars)
#[test]
fn test_adversarial_utf8_4byte_chars_at_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    // 😀 = F0 9F 98 80 (4 bytes). Two of them = 8 bytes = max
    let data = build_dynamic_account_data(&[0xF0, 0x9F, 0x98, 0x80, 0xF0, 0x9F, 0x98, 0x80], &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "two 4-byte emoji chars at exact max=8 must be accepted: {:?}",
        result.program_result
    );
}

// ============================================================================
// ADVERSARIAL TESTS: SmallPrefix (u8) Attack Surface
// ============================================================================

/// u8 prefix = 255 (max=100 for tag) — tests u8 max check
#[test]
fn test_adversarial_small_prefix_u8_max_value() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + u8 tag prefix(255) + 0 bytes of tag data + u16 scores prefix(0)
    let mut data = vec![SMALL_PREFIX_DISC, 255];
    // No actual tag data — prefix claims 255 bytes but max=100
    data.extend_from_slice(&0u16.to_le_bytes()); // scores count = 0

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![23], // small_prefix_check discriminator
    };
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "u8 prefix=255 (max=100) must be rejected"
    );
}

/// u16 scores count = 255 (max=10) — vec prefix overflow
#[test]
fn test_adversarial_small_prefix_vec_u8_overflow() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + u8 tag(prefix=0, empty) + u16 scores(prefix=255, max=10)
    let data = vec![SMALL_PREFIX_DISC, 0, 255, 0];

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![23],
    };
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "u8 vec count=255 (max=10) must be rejected"
    );
}

// ============================================================================
// ADVERSARIAL TESTS: Mutation → Readback Correctness
// ============================================================================

/// Grow name with 2 trailing tags: verifies memmove shifts tags correctly
/// and accessor reads them back at the new offset
#[test]
fn test_adversarial_mutate_grow_then_readback_tags() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"ab", &[tag1, tag2]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Grow name from "ab" (2) to "12345678" (8=max), expect 2 tags preserved
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 2, b"12345678");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "grow name then readback tags should succeed: {:?}",
        result.program_result
    );

    // Also verify the raw bytes to be extra paranoid (compact layout)
    let rd = &result.resulting_accounts[0].1.data;
    assert_eq!(rd[0], DYNAMIC_ACCOUNT_DISC);
    // Compact header: [name_len(1)][tags_count(2)]
    let name_len = rd[1] as usize;
    assert_eq!(name_len, 8);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&rd[4..12], b"12345678");
    assert_eq!(&rd[12..44], tag1.as_ref());
    assert_eq!(&rd[44..76], tag2.as_ref());
}

/// Shrink name with trailing tags: verifies memmove on shrink
#[test]
fn test_adversarial_mutate_shrink_then_readback_tags() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"12345678", &[tag1, tag2]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Shrink name from "12345678" (8) to "x" (1), expect 2 tags preserved
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 2, b"x");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "shrink name then readback tags should succeed: {:?}",
        result.program_result
    );

    let rd = &result.resulting_accounts[0].1.data;
    // Compact header: [name_len(1)][tags_count(2)]
    let name_len = rd[1] as usize;
    assert_eq!(name_len, 1);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&rd[4..5], b"x");
    assert_eq!(&rd[5..37], tag1.as_ref());
    assert_eq!(&rd[37..69], tag2.as_ref());
}

/// Mutate name to empty with trailing tags: edge case for zero-length memmove
/// source
#[test]
fn test_adversarial_mutate_to_empty_then_readback_tags() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"hello", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 1, b"");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "mutate to empty then readback tags should succeed: {:?}",
        result.program_result
    );

    let rd = &result.resulting_accounts[0].1.data;
    let name_len = rd[1] as usize;
    assert_eq!(name_len, 0);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    assert_eq!(&rd[4..36], tag.as_ref());
}

/// Grow from empty to max: maximum realloc + memmove distance
#[test]
fn test_adversarial_mutate_empty_to_max_then_readback() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Grow name from "" (0) to "12345678" (8=max)
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 1, b"12345678");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "grow from empty to max then readback should succeed: {:?}",
        result.program_result
    );

    let rd = &result.resulting_accounts[0].1.data;
    let name_len = rd[1] as usize;
    assert_eq!(name_len, 8);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&rd[4..12], b"12345678");
    assert_eq!(&rd[12..44], tag.as_ref());
}

// ============================================================================
// ADVERSARIAL TESTS: Sequential Mutation Stress
// ============================================================================

/// Sequential mutations: grow→shrink→grow in a single transaction chain.
/// Tests that repeated realloc + memmove doesn't corrupt state.
/// We do this as separate Mollusk calls since each produces new account state.
#[test]
fn test_adversarial_sequential_mutations_grow_shrink_grow() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();

    // Start: name="ab", 2 tags
    let account_bytes = build_dynamic_account_data(b"ab", &[tag1, tag2]);
    let mut current_account = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    // Step 1: Grow "ab" → "12345678" (max)
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 2, b"12345678");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, current_account.clone()),
            (payer, payer_account.clone()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "step 1 (grow) failed: {:?}",
        result.program_result
    );
    current_account = result.resulting_accounts[0].1.clone();

    // Step 2: Shrink "12345678" → "x"
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 2, b"x");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, current_account.clone()),
            (payer, result.resulting_accounts[1].1.clone()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "step 2 (shrink) failed: {:?}",
        result.program_result
    );
    current_account = result.resulting_accounts[0].1.clone();

    // Step 3: Grow again "x" → "abcdef" (6 bytes, not max)
    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 2, b"abcdef");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, current_account),
            (payer, result.resulting_accounts[1].1.clone()),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "step 3 (re-grow) failed: {:?}",
        result.program_result
    );

    // Final byte-level verification (compact layout)
    let rd = &result.resulting_accounts[0].1.data;
    // Compact header: [name_len(1)][tags_count(2)]
    let name_len = rd[1] as usize;
    assert_eq!(name_len, 6);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&rd[4..10], b"abcdef");
    assert_eq!(
        &rd[10..42],
        tag1.as_ref(),
        "tag1 corrupted after 3 mutations"
    );
    assert_eq!(
        &rd[42..74],
        tag2.as_ref(),
        "tag2 corrupted after 3 mutations"
    );
}

/// Mutate to same name (no-op path): verifies no data corruption
#[test]
fn test_adversarial_mutate_noop_same_name() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"hello", &[tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes.clone(),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction =
        build_mutate_then_readback_instruction(account, payer, system_program, 1, b"hello");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "no-op mutation should succeed: {:?}",
        result.program_result
    );

    // Data should be byte-identical
    assert_eq!(
        &result.resulting_accounts[0].1.data, &account_bytes,
        "no-op mutation must not change any bytes"
    );
}

#[test]
fn test_dynamic_view_mut_replaces_name_and_tags() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let old_tag = Address::new_unique();
    let new_tag1 = Address::new_unique();
    let new_tag2 = Address::new_unique();

    let account_bytes = build_dynamic_account_data(b"old", &[old_tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction = build_dynamic_view_mut_instruction(
        account,
        payer,
        system_program,
        b"replace",
        &[new_tag1, new_tag2],
    );
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "dynamic view replacement should succeed: {:?}",
        result.program_result
    );

    let rd = &result.resulting_accounts[0].1.data;
    // Compact header: [name_len(1)][tags_count(2)]
    assert_eq!(rd[1] as usize, 7);
    let tags_count = u16::from_le_bytes(rd[2..4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    // Compact tail: [name_bytes][tag_bytes]
    assert_eq!(&rd[4..11], b"replace");
    assert_eq!(&rd[11..43], new_tag1.as_ref());
    assert_eq!(&rd[43..75], new_tag2.as_ref());
}

#[test]
fn test_as_mut_preserves_untouched_fields() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let account = Address::new_unique();
    let payer = Address::new_unique();

    let old_tag = Address::new_unique();
    let account_bytes = build_dynamic_account_data(b"old", &[old_tag]);
    let account_data = Account {
        lamports: 1_000_000,
        data: account_bytes.clone(),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let instruction =
        build_dynamic_view_mut_missing_field_instruction(account, payer, system_program, b"rename");
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_data),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "as_mut partial mutation should succeed: {:?}",
        result.program_result
    );

    // Verify: name changed to "rename", tags preserved (1 tag = old_tag)
    let expected = build_dynamic_account_data(b"rename", &[old_tag]);
    assert_eq!(
        result.resulting_accounts[0].1.data, expected,
        "as_mut must update name and preserve untouched tags"
    );
}

// ============================================================================
// ADVERSARIAL TESTS: Validation Boundary Conditions
// ============================================================================

/// Account with just a discriminator byte and nothing else
#[test]
fn test_adversarial_disc_only_no_fields() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = vec![DYNAMIC_ACCOUNT_DISC]; // just disc, no prefix bytes at all

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::AccountDataTooSmall),
        "disc-only account must fail (can't read first prefix)"
    );
}

/// Account with name prefix but no tags prefix at all
#[test]
fn test_adversarial_missing_second_prefix() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + u8 name prefix(0) = valid empty name, but no u16 tags prefix follows
    let data = vec![DYNAMIC_ACCOUNT_DISC, 0];

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::AccountDataTooSmall),
        "missing second field prefix must fail"
    );
}

/// Partial u16 tags prefix: name is empty (u8=0) but only 1 of 2 tag prefix
/// bytes present
#[test]
fn test_adversarial_partial_u32_prefix() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + u8 name(0) + 1 byte of u16 tags prefix (need 2)
    let data = vec![DYNAMIC_ACCOUNT_DISC, 0x00, 0x00];

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::AccountDataTooSmall),
        "partial u16 tags prefix must fail"
    );
}

/// MixedAccount: valid fixed fields but label prefix extends past end
#[test]
fn test_adversarial_mixed_fixed_valid_dynamic_truncated() {
    let mollusk = setup();
    let account = Address::new_unique();

    let authority = Address::new_unique();
    // disc(1) + authority(32) + value(8) = 41 bytes of fixed data
    // Then u8 label prefix claims 10 bytes but only 2 are present
    let mut data = vec![0u8; 41 + 1 + 2]; // 44 bytes total
    data[0] = MIXED_ACCOUNT_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&42u64.to_le_bytes());
    data[41] = 10; // u8 prefix claims 10 bytes
    data[42..44].copy_from_slice(b"ab"); // only 2 present

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![22], // mixed_account_check discriminator
    };
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "truncated dynamic field after valid fixed section must fail"
    );
}

/// MixedAccount: fixed section truncated (only 20 of 40 ZC bytes)
#[test]
fn test_adversarial_mixed_fixed_section_truncated() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc(1) + 20 bytes (need 40 for Address+u64)
    let mut data = vec![0u8; 21];
    data[0] = MIXED_ACCOUNT_DISC;

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![22],
    };
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "truncated fixed section must be rejected"
    );
}

/// All-zero account data (0-byte discriminator = potential uninitialized
/// attack)
#[test]
fn test_adversarial_all_zeros_account() {
    let mollusk = setup();
    let account = Address::new_unique();

    // 100 bytes of zeros — discriminator 0 is banned
    let data = vec![0u8; 100];

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Try with DynamicAccountCheck (disc=5)
    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "all-zero account must be rejected (wrong discriminator)"
    );
}

/// Account with correct disc and valid prefixes but extra trailing garbage
#[test]
fn test_adversarial_trailing_garbage_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Valid account + 50 bytes of garbage at the end
    let mut data = build_dynamic_account_data(b"hi", &[]);
    data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF].repeat(12));

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "trailing garbage after valid fields should be accepted (ignored): {:?}",
        result.program_result
    );
}

/// Name = exactly 8 bytes of valid UTF-8 + tags = exactly 2 elements:
/// Both fields at their maximums simultaneously
#[test]
fn test_adversarial_all_fields_at_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    let tag1 = Address::new_unique();
    let tag2 = Address::new_unique();
    let data = build_dynamic_account_data(b"abcdefgh", &[tag1, tag2]);

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "both fields at max should be accepted: {:?}",
        result.program_result
    );

    // Also verify via readback
    let instruction = build_readback_instruction(account, 8, 2);
    let account_data2 = Account {
        lamports: 1_000_000,
        data: build_dynamic_account_data(b"abcdefgh", &[tag1, tag2]),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let result = mollusk.process_instruction(&instruction, &[(account, account_data2)]);
    assert!(
        result.program_result.is_ok(),
        "readback at max should succeed: {:?}",
        result.program_result
    );
}

/// Both fields empty: minimum valid account
#[test]
fn test_adversarial_minimum_valid_account() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc(1) + u8 name_prefix(1, len=0) + u16 tags_prefix(2, count=0) = 4 bytes
    let data = build_dynamic_account_data(b"", &[]);
    assert_eq!(data.len(), 4, "minimum valid account should be 4 bytes");

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = DynamicAccountCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "minimum valid account (both fields empty) should pass: {:?}",
        result.program_result
    );
}

// ============================================================================
// TAIL FIELD TESTS: &str and &[u8] tail fields
// ============================================================================

#[test]
fn test_dyn_str_valid_utf8_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_dyn_str_account_data(authority, b"hello");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid UTF-8 tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dyn_str_empty_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_dyn_str_account_data(authority, b"");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_str_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dyn_str_multibyte_utf8_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    // "café" = 63 61 66 C3 A9 = 5 bytes
    let data = build_dyn_str_account_data(authority, "café".as_bytes());
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid multi-byte UTF-8 tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dyn_bytes_valid_data_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_dyn_bytes_account_data(authority, &[0xFF, 0x00, 0xAB, 0xCD]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_bytes_check_instruction(account, 4);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid tail bytes should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dyn_bytes_empty_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_dyn_bytes_account_data(authority, &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_bytes_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty tail bytes should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_dyn_str_wrong_discriminator_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let mut data = build_dyn_str_account_data(authority, b"hello");
    data[0] = 99; // wrong discriminator

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "wrong discriminator must be rejected"
    );
}

#[test]
fn test_dyn_str_truncated_fixed_section_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc(1) + only 16 bytes (need 32 for Address)
    let mut data = vec![0u8; 17];
    data[0] = DYN_STR_DISC;

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_dyn_str_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "truncated fixed section must be rejected"
    );
}

// ============================================================================
// Adversarial Tests — Attacker-Controlled Inputs
// ============================================================================

/// Send completely empty instruction data (0 bytes) — no discriminator at all.
/// The dispatch macro should reject this because ix_data.len() <
/// discriminator_len.
#[test]
fn test_adversarial_ix_data_empty() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new(signer, true)],
        data: vec![],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "empty instruction data (no discriminator) must be rejected, not crash or read OOB"
    );
}

/// Send 1 byte that does NOT match any known discriminator.
/// Even for 1-byte discriminator instructions, an unrecognized value should
/// fail.
#[test]
fn test_adversarial_ix_data_one_byte_unknown_disc() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new(signer, true)],
        data: vec![255],
    };
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "unrecognized 1-byte discriminator must be rejected"
    );
}

/// Instruction discriminator=21 (dynamic_instruction_check) takes a String<8>.
/// Craft raw data where the u8 string prefix claims 255 bytes but only 3
/// bytes of actual data follow. The framework must reject this, not read OOB.
/// (String<N> instruction args use u8 prefix on the wire.)
#[test]
fn test_adversarial_ix_dynamic_string_prefix_overflow_u32_max() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let mut data = vec![21u8]; // discriminator for dynamic_instruction_check
    data.push(u8::MAX); // u8 prefix claiming 255 bytes
    data.extend_from_slice(b"abc");

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "string prefix claiming 255 bytes with only 3 bytes present must be rejected"
    );
}

/// Instruction discriminator=21: string prefix=200 but only 10 bytes of data
/// follow. Slightly above actual data — subtler than u8::MAX.
/// (String<N> instruction args use u8 prefix on the wire.)
#[test]
fn test_adversarial_ix_dynamic_string_prefix_overflow_1024() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let mut data = vec![21u8];
    data.push(200u8); // u8 prefix claiming 200 bytes
    data.extend_from_slice(b"0123456789");

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "string prefix=200 with only 10 bytes present must be rejected"
    );
}

/// Instruction discriminator=21: string prefix=0 (empty string).
/// This is technically valid — the handler receives an empty &str.
/// (String<N> instruction args use u8 prefix on the wire.)
#[test]
fn test_adversarial_ix_dynamic_string_prefix_zero() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let mut data = vec![21u8];
    data.push(0u8); // u8 prefix: empty string

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_ok(),
        "string prefix=0 (empty string) should be valid: {:?}",
        result.program_result
    );
}

/// Send a valid instruction (discriminator=21, valid String<8>) but append 100
/// bytes of random garbage at the end. Does the program ignore them or reject?
/// (String<N> instruction args use u8 prefix on the wire.)
#[test]
fn test_adversarial_ix_data_with_extra_trailing_garbage() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let mut data = vec![21u8]; // dynamic_instruction_check
    data.push(5u8); // u8 prefix: name len = 5
    data.extend_from_slice(b"hello");
    data.extend_from_slice(&[0xDE; 100]);

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };

    let _result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    // The framework may accept this (trailing data ignored) or reject.
    // Either is acceptable — the key thing is it must NOT crash or read OOB.
    // We do NOT assert pass/fail here — we assert no panic/abort occurred.
}

/// Instruction with discriminator only, no args, for an instruction that
/// expects args. discriminator=21 expects String<8> but we only send the
/// discriminator byte.
#[test]
fn test_adversarial_ix_disc_only_missing_args() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data: vec![21], // disc only, no string prefix
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "instruction with disc only (missing args) must be rejected"
    );
}

/// Instruction discriminator=21: string prefix claims length > max (8).
/// prefix=9, data has 9 valid UTF-8 bytes. Should be rejected by max check.
/// (String<N> instruction args use u8 prefix on the wire.)
#[test]
fn test_adversarial_ix_string_exceeds_max() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let mut data = vec![21u8];
    data.push(9u8); // u8 prefix: 9 bytes exceeds max of 8
    data.extend_from_slice(b"123456789");

    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(signer, true)],
        data,
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_err(),
        "instruction string length=9 (max=8) must be rejected"
    );
}
