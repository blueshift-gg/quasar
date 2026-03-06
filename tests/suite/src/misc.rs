use mollusk_svm::result::ProgramResult;
use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use quasar_core::error::QuasarError;
use quasar_core::prelude::ProgramError;
use quasar_test_misc::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

const SIMPLE_ACCOUNT_SIZE: usize = 42; // 1 disc + 32 addr + 8 u64 + 1 u8
const MULTI_DISC_SIZE: usize = 10; // 2 disc + 8 u64
const DYNAMIC_ACCOUNT_DISC: u8 = 5;
const DYNAMIC_HEADER_SIZE: usize = 1; // disc only (no fixed ZC fields)
const MIXED_ACCOUNT_DISC: u8 = 6;
const MIXED_FIXED_SIZE: usize = 32 + 8; // Address + u64
const SMALL_PREFIX_DISC: u8 = 7;

fn build_simple_account_data(authority: Address, value: u64, bump: u8) -> Vec<u8> {
    let mut data = vec![0u8; 42];
    data[0] = 1; // SimpleAccount discriminator
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data[41] = bump;
    data
}

fn build_multi_disc_account_data(value: u64) -> Vec<u8> {
    let mut data = vec![0u8; 10];
    data[0] = 1; // MultiDiscAccount discriminator byte 0
    data[1] = 2; // MultiDiscAccount discriminator byte 1
    data[2..10].copy_from_slice(&value.to_le_bytes());
    data
}

fn build_dynamic_account_data(name: &[u8], tags: &[Address]) -> Vec<u8> {
    // Inline prefix layout: [disc][u32:name_len][name_bytes][u32:tags_count][tag_elements]
    let name_len = name.len();
    let tags_count = tags.len();
    let tags_bytes = tags_count * 32;
    let total = DYNAMIC_HEADER_SIZE + 4 + name_len + 4 + tags_bytes;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = DYNAMIC_ACCOUNT_DISC;
    offset += 1;

    // name: u32 prefix (byte length) + data
    data[offset..offset + 4].copy_from_slice(&(name_len as u32).to_le_bytes());
    offset += 4;
    data[offset..offset + name_len].copy_from_slice(name);
    offset += name_len;

    // tags: u32 prefix (element count) + elements
    data[offset..offset + 4].copy_from_slice(&(tags_count as u32).to_le_bytes());
    offset += 4;
    for (i, tag) in tags.iter().enumerate() {
        data[offset + i * 32..offset + (i + 1) * 32].copy_from_slice(tag.as_ref());
    }

    data
}

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_misc::ID,
        "../../target/deploy/quasar_test_misc",
    )
}

// ============================================================================
// Account Init (tests 1-8)
// ============================================================================

#[test]
fn test_init_success() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init failed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), SIMPLE_ACCOUNT_SIZE, "data length");
    assert_eq!(data[0], 1, "discriminator");
    assert_eq!(&data[1..33], payer.as_ref(), "authority = payer");
    assert_eq!(&data[33..41], &42u64.to_le_bytes(), "value = 42");
    assert_eq!(
        result.resulting_accounts[1].1.owner,
        quasar_test_misc::ID,
        "owner"
    );

    println!("  init_success CU: {}", result.compute_units_consumed);
}

#[test]
fn test_init_wrong_payer_not_signer() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let mut instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    // Remove signer flag from payer
    instruction.accounts[0].is_signer = false;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should fail when payer is not signer"
    );
}

#[test]
fn test_init_insufficient_lamports() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(1, 0, &system_program); // Almost no lamports

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should fail with insufficient lamports"
    );
}

#[test]
fn test_init_reinit_attack() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Account already initialized with correct data
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(payer, 100, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should fail on already-initialized account (reinit attack)"
    );
}

#[test]
fn test_init_all_zero_data() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Account with all-zero data but owned by our program (simulates attack)
    let account_obj = Account {
        lamports: 1_000_000,
        data: vec![0u8; SIMPLE_ACCOUNT_SIZE],
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should reject account with all-zero data owned by program"
    );
}

#[test]
fn test_init_wrong_space() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Account with data too small (already allocated but wrong size)
    let account_obj = Account {
        lamports: 1_000_000,
        data: vec![1u8, 0, 0], // discriminator + too few bytes
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitializeInstruction {
        payer,
        account,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should fail when account data is too small"
    );
}

#[test]
fn test_init_wrong_pda_seeds() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (wrong_pda, _) =
        Address::find_program_address(&[b"wrong_seed", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = InitializeInstruction {
        payer,
        account: wrong_pda,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (wrong_pda, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init should fail when account address doesn't match seeds [b\"simple\", payer]"
    );
}

#[test]
fn test_init_if_needed_new() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::new(0, 0, &system_program); // Uninitialized

    let instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 99,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed (new) failed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data[0], 1, "discriminator");
    assert_eq!(&data[33..41], &99u64.to_le_bytes(), "value = 99");
}

#[test]
fn test_init_if_needed_existing() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Already initialized with correct owner and discriminator
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(payer, 100, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 200,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed (existing) failed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(&data[33..41], &200u64.to_le_bytes(), "value updated to 200");

    assert_eq!(
        result.resulting_accounts[0].1.lamports, 10_000_000_000,
        "payer lamports should be unchanged (no rent payment for existing account)"
    );
    assert_eq!(
        result.resulting_accounts[1].1.lamports, 1_000_000,
        "account lamports should be unchanged (no re-creation)"
    );
}

// ============================================================================
// Account Close (tests 9-12)
// ============================================================================

#[test]
fn test_close_success() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_lamports = 2_000_000u64;
    let account_obj = Account {
        lamports: account_lamports,
        data: build_simple_account_data(authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CloseAccountInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account.clone()),
            (account, account_obj),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "close failed: {:?}",
        result.program_result
    );

    let closed_account = &result.resulting_accounts[1].1;
    assert_eq!(closed_account.lamports, 0, "closed account lamports = 0");
    assert_eq!(
        closed_account.owner,
        Address::default(),
        "owner reassigned to system"
    );
}

#[test]
fn test_close_wrong_authority() {
    let mollusk = setup();

    let real_authority = Address::new_unique();
    let fake_authority = Address::new_unique();
    let fake_authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", fake_authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 2_000_000,
        data: build_simple_account_data(real_authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CloseAccountInstruction {
        authority: fake_authority,
        account,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (fake_authority, fake_authority_account),
            (account, account_obj),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "close should fail with wrong authority"
    );
}

#[test]
fn test_close_verify_zeroed() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 2_000_000,
        data: build_simple_account_data(authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CloseAccountInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(result.program_result.is_ok());

    let closed = &result.resulting_accounts[1].1;
    assert_eq!(closed.data.len(), 0, "data resized to 0");
}

#[test]
fn test_close_lamports_transferred() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_lamports = 1_000_000u64;
    let authority_account = Account::new(authority_lamports, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_lamports = 2_000_000u64;
    let account_obj = Account {
        lamports: account_lamports,
        data: build_simple_account_data(authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CloseAccountInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(result.program_result.is_ok());

    let authority_after = result.resulting_accounts[0].1.lamports;
    assert_eq!(
        authority_after,
        authority_lamports + account_lamports,
        "authority receives closed account lamports"
    );
}

// ============================================================================
// Constraint: has_one (tests 13-16)
// ============================================================================

#[test]
fn test_has_one_success() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_ok(),
        "has_one should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_has_one_wrong_authority() {
    let mollusk = setup();

    let real_authority = Address::new_unique();
    let fake_authority = Address::new_unique();
    let fake_authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", fake_authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(real_authority, 42, bump), // Authority stored = real
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction {
        authority: fake_authority, // But passing fake
        account,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (fake_authority, fake_authority_account),
            (account, account_obj),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail with wrong authority"
    );
}

#[test]
fn test_has_one_zeroed_authority() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    // Stored authority is all-zero
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::default(), 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when stored authority is all-zero"
    );
}

#[test]
fn test_has_one_single_bit_diff() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);

    // Create authority that differs by 1 bit
    let mut wrong_bytes = authority.to_bytes();
    wrong_bytes[0] ^= 1;
    let wrong_authority = Address::new_from_array(wrong_bytes);

    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(wrong_authority, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateHasOneInstruction { authority, account }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, authority_account), (account, account_obj)],
    );

    assert!(
        result.program_result.is_err(),
        "has_one should fail when authority differs by 1 bit"
    );
}

// ============================================================================
// Constraint: address (tests 17-19)
// ============================================================================

#[test]
fn test_address_success() {
    let mollusk = setup();

    let target = quasar_test_misc::EXPECTED_ADDRESS;
    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "address check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_address_wrong() {
    let mollusk = setup();

    let wrong_target = Address::new_unique();
    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction {
        target: wrong_target,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(wrong_target, target_account)]);

    assert!(
        result.program_result.is_err(),
        "address check should fail with wrong address"
    );
}

#[test]
fn test_address_with_constant() {
    let mollusk = setup();

    // Verify that the const address is the expected value
    let target = Address::new_from_array([42u8; 32]);
    assert_eq!(target, quasar_test_misc::EXPECTED_ADDRESS);

    let target_account = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = UpdateAddressInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "const address should work: {:?}",
        result.program_result
    );
}

// ============================================================================
// Constraint: signer (tests 20-22)
// ============================================================================

#[test]
fn test_signer_success() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SignerCheckInstruction { signer }.into();

    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    assert!(
        result.program_result.is_ok(),
        "signer check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_signer_not_signer() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let signer_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = SignerCheckInstruction { signer }.into();
    instruction.accounts[0].is_signer = false;

    let result = mollusk.process_instruction(&instruction, &[(signer, signer_account)]);

    assert!(
        result.program_result.is_err(),
        "signer check should fail when not signer"
    );
}

// ============================================================================
// Constraint: owner (tests 22-24)
// ============================================================================

#[test]
fn test_owner_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID, // Correct owner
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "owner check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_owner_wrong_program() {
    let mollusk = setup();

    let account = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: wrong_owner, // Wrong owner
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "owner check should fail with wrong program"
    );
}

#[test]
fn test_owner_system_program() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: Address::default(), // System program (uninitialized)
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "owner check should fail when owned by system program"
    );
}

// ============================================================================
// Constraint: mut (tests 26-28)
// ============================================================================

#[test]
fn test_mut_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "mut check should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_mut_not_writable() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 100,
    }
    .into();

    // Make account read-only
    instruction.accounts[0] = solana_instruction::AccountMeta::new_readonly(account, false);

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "mut check should fail when account is not writable"
    );
}

#[test]
fn test_mut_writes_persist() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = MutCheckInstruction {
        account,
        new_value: 999,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(result.program_result.is_ok());

    let data = &result.resulting_accounts[0].1.data;
    assert_eq!(
        &data[33..41],
        &999u64.to_le_bytes(),
        "written value should persist"
    );
}

// ============================================================================
// SystemProgram CPI (tests 29-32)
// ============================================================================

#[test]
fn test_create_account() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let new_account = Address::new_unique();
    let new_account_obj = Account::new(0, 0, &system_program);

    let owner = Address::new_unique();
    let space = 64u64;
    let lamports = 1_000_000u64;

    let instruction: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program,
        lamports,
        space,
        owner,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (new_account, new_account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "create_account failed: {:?}",
        result.program_result
    );

    let created = &result.resulting_accounts[1].1;
    assert_eq!(created.lamports, lamports, "lamports");
    assert_eq!(created.data.len(), space as usize, "space");
    assert_eq!(created.owner, owner, "owner");
}

#[test]
fn test_transfer() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let from_account = Account::new(10_000_000_000, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(1_000_000, 0, &system_program);

    let amount = 5_000_000_000u64;

    let instruction: Instruction = TransferTestInstruction {
        from,
        to,
        system_program,
        amount,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (from, from_account),
            (to, to_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "transfer failed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.lamports,
        10_000_000_000 - amount,
        "from lamports"
    );
    assert_eq!(
        result.resulting_accounts[1].1.lamports,
        1_000_000 + amount,
        "to lamports"
    );
}

#[test]
fn test_transfer_zero() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let from_account = Account::new(1_000_000, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(1_000_000, 0, &system_program);

    let instruction: Instruction = TransferTestInstruction {
        from,
        to,
        system_program,
        amount: 0,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (from, from_account),
            (to, to_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "zero transfer should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_assign() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let account = Address::new_unique();
    let account_obj = Account::new(1_000_000, 0, &system_program);

    let new_owner = Address::new_unique();

    let instruction: Instruction = AssignTestInstruction {
        account,
        system_program,
        owner: new_owner,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "assign failed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.owner, new_owner,
        "owner changed"
    );
}

// ============================================================================
// SystemAccount (tests 33-34)
// ============================================================================

#[test]
fn test_system_account_success() {
    let mollusk = setup();

    let target = Address::new_unique();
    let target_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = SystemAccountCheckInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_ok(),
        "system account check should pass for system-owned account: {:?}",
        result.program_result
    );
}

#[test]
fn test_system_account_wrong_owner() {
    let mollusk = setup();

    let target = Address::new_unique();
    let wrong_owner = Address::new_unique();
    let target_account = Account {
        lamports: 1_000_000,
        data: Vec::new(),
        owner: wrong_owner,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = SystemAccountCheckInstruction { target }.into();

    let result = mollusk.process_instruction(&instruction, &[(target, target_account)]);

    assert!(
        result.program_result.is_err(),
        "system account check should fail when owner is not system program"
    );
}

// ============================================================================
// init_if_needed Adversarial (tests 35-38)
// ============================================================================

#[test]
fn test_init_if_needed_wrong_owner() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Existing account with wrong owner
    let wrong_owner = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(payer, 42, bump),
        owner: wrong_owner,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 99,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_if_needed should fail with wrong owner"
    );
}

#[test]
fn test_init_if_needed_wrong_discriminator() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Existing account with wrong discriminator
    let mut data = vec![0u8; SIMPLE_ACCOUNT_SIZE];
    data[0] = 99; // Wrong discriminator (should be 1)
    let account_obj = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 99,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_if_needed should fail with wrong discriminator"
    );
}

#[test]
fn test_init_if_needed_data_too_small() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    // Existing account with data too small
    let account_obj = Account {
        lamports: 1_000_000,
        data: vec![1u8], // Only discriminator, no fields
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 99,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_if_needed should fail when data too small"
    );
}

#[test]
fn test_init_if_needed_not_writable() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(payer, 42, bump),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = InitIfNeededInstruction {
        payer,
        account,
        system_program,
        value: 99,
    }
    .into();

    // Make account read-only
    instruction.accounts[1] = solana_instruction::AccountMeta::new_readonly(account, false);

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_if_needed should fail when account not writable"
    );
}

// ============================================================================
// Discriminator Validation (tests 37-38)
// ============================================================================

#[test]
fn test_wrong_discriminator() {
    let mollusk = setup();

    let account = Address::new_unique();
    let mut data = vec![0u8; SIMPLE_ACCOUNT_SIZE];
    data[0] = 2; // Wrong: SimpleAccount expects 1
    let account_obj = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "should fail with wrong discriminator"
    );
}

#[test]
fn test_check_multi_disc_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let data = build_multi_disc_account_data(42);
    let account_obj = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CheckMultiDiscInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "multi-byte discriminator account should validate successfully"
    );
}

#[test]
fn test_partial_discriminator_match() {
    let mollusk = setup();

    let account = Address::new_unique();
    // MultiDiscAccount expects discriminator [1, 2]. Provide [1, 0] — partial match.
    let mut data = vec![0u8; MULTI_DISC_SIZE];
    data[0] = 1; // First byte matches
    data[1] = 0; // Second byte doesn't match (should be 2)
    let account_obj = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = CheckMultiDiscInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "should fail with partial discriminator match"
    );
}

// ============================================================================
// Constraint Check
// ============================================================================

#[test]
fn test_constraint_success() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 100, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_ok(),
        "constraint should pass when value > 0: {:?}",
        result.program_result
    );
}

#[test]
fn test_constraint_fail_zero_value() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 0, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = ConstraintCheckInstruction { account }.into();

    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "constraint should fail when value == 0"
    );
}

// ============================================================================
// Realloc Check
// ============================================================================

#[test]
fn test_realloc_grow() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let new_space = 100u64;
    let instruction: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program,
        _new_space: new_space,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_obj),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "realloc grow should succeed: {:?}",
        result.program_result
    );

    let resulting = &result.resulting_accounts[0].1;
    assert_eq!(
        resulting.data.len(),
        new_space as usize,
        "data should be resized"
    );
}

#[test]
fn test_realloc_shrink() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let account = Address::new_unique();
    let mut data = build_simple_account_data(Address::new_unique(), 42, 0);
    data.resize(100, 0);
    let account_obj = Account {
        lamports: 10_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let new_space = SIMPLE_ACCOUNT_SIZE as u64;
    let instruction: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program,
        _new_space: new_space,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_obj),
            (payer, payer_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "realloc shrink should succeed: {:?}",
        result.program_result
    );

    let resulting = &result.resulting_accounts[0].1;
    assert_eq!(
        resulting.data.len(),
        SIMPLE_ACCOUNT_SIZE,
        "data should shrink back to original size"
    );
}

// ============================================================================
// Optional Account (discriminator 15)
// ============================================================================

#[test]
fn test_optional_account_with_some() {
    let mollusk = setup();
    let required = Address::new_unique();
    let optional = Address::new_unique();

    let required_data = build_simple_account_data(Address::new_unique(), 42, 0);
    let optional_data = build_simple_account_data(Address::new_unique(), 7, 0);

    let required_account = Account {
        lamports: 1_000_000,
        data: required_data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };
    let optional_account = Account {
        lamports: 1_000_000,
        data: optional_data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OptionalAccountInstruction { required, optional }.into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(required, required_account), (optional, optional_account)],
    );

    assert!(
        result.program_result.is_ok(),
        "optional account with Some should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_optional_account_with_none() {
    let mollusk = setup();
    let required = Address::new_unique();
    let program_id = quasar_test_misc::ID;

    let required_data = build_simple_account_data(Address::new_unique(), 42, 0);
    let required_account = Account {
        lamports: 1_000_000,
        data: required_data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OptionalAccountInstruction {
        required,
        optional: program_id,
    }
    .into();

    let result = mollusk.process_instruction(&instruction, &[(required, required_account)]);

    assert!(
        result.program_result.is_ok(),
        "optional account with None (program ID) should succeed: {:?}",
        result.program_result
    );
}

// ============================================================================
// Remaining Accounts (discriminator 16)
// ============================================================================

#[test]
fn test_remaining_accounts_with_extras() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let extra1 = Address::new_unique();
    let extra2 = Address::new_unique();

    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let extra1_account = Account::new(1_000_000, 0, &Address::default());
    let extra2_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(extra1, false));
    instruction
        .accounts
        .push(solana_instruction::AccountMeta::new_readonly(extra2, false));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (extra1, extra1_account),
            (extra2, extra2_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with extras should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_accounts_empty() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();

    let result = mollusk.process_instruction(&instruction, &[(authority, authority_account)]);

    assert!(
        result.program_result.is_ok(),
        "remaining accounts with no extras should succeed: {:?}",
        result.program_result
    );
}

#[test]
fn test_remaining_accounts_overflow_errors() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let mut instruction: Instruction = RemainingAccountsCheckInstruction { authority }.into();
    let mut accounts = vec![(authority, authority_account)];

    for _ in 0..=64 {
        let addr = Address::new_unique();
        instruction
            .accounts
            .push(solana_instruction::AccountMeta::new_readonly(addr, false));
        accounts.push((addr, Account::new(1_000_000, 0, &Address::default())));
    }

    let result = mollusk.process_instruction(&instruction, &accounts);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::Custom(
            QuasarError::RemainingAccountsOverflow as u32
        ))
    );
}

#[test]
fn test_dynamic_account_invalid_utf8_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(&[0xFF], &[]);
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
fn test_dynamic_instruction_invalid_utf8_rejected() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = DynamicInstructionCheckInstruction {
        authority,
        name: vec![0xFF],
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

    // Corrupt name length prefix (at offset 1..5) to exceed max of 8
    data[1..5].copy_from_slice(&100u32.to_le_bytes());

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
    data.truncate(DYNAMIC_HEADER_SIZE + 4); // just disc + u32 prefix, no bytes

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
// Space Override (#[account(init, space = 100)])
// ============================================================================

#[test]
fn test_space_override_allocates_custom_size() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"spacetest", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = SpaceOverrideInstruction {
        payer,
        account,
        system_program,
        value: 77,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "space override init should succeed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(
        data.len(),
        100,
        "account should be allocated with space = 100"
    );
    assert_eq!(data[0], 1, "discriminator should be set");
    assert_eq!(
        result.resulting_accounts[1].1.owner,
        quasar_test_misc::ID,
        "owner should be program"
    );
}

// ============================================================================
// Explicit Payer (#[account(init, payer = funder)])
// ============================================================================

#[test]
fn test_explicit_payer_success() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let funder = Address::new_unique();
    let funder_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"explicit", funder.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = ExplicitPayerInstruction {
        funder,
        account,
        system_program,
        value: 55,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (funder, funder_account),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "explicit payer init should succeed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data[0], 1, "discriminator");
    assert_eq!(&data[1..33], funder.as_ref(), "authority = funder");
    assert_eq!(&data[33..41], &55u64.to_le_bytes(), "value = 55");
    assert_eq!(
        result.resulting_accounts[1].1.owner,
        quasar_test_misc::ID,
        "owner"
    );
}

// ============================================================================
// Optional Account with has_one constraint (discriminator 19)
// ============================================================================

#[test]
fn test_optional_has_one_some_valid() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_simple_account_data(authority, 42, 0);

    let instruction: Instruction = OptionalHasOneInstruction {
        authority,
        account: account_addr,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_misc::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "optional has_one with valid authority should pass: {:?}",
        result.program_result
    );
}

#[test]
fn test_optional_has_one_some_wrong() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let account_addr = Address::new_unique();
    let account_data = build_simple_account_data(wrong_authority, 42, 0);

    let instruction: Instruction = OptionalHasOneInstruction {
        authority,
        account: account_addr,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                account_addr,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_misc::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "optional has_one with wrong authority should fail"
    );
}

#[test]
fn test_optional_has_one_none() {
    let mollusk = setup();
    let authority = Address::new_unique();

    let instruction: Instruction = OptionalHasOneInstruction {
        authority,
        account: quasar_test_misc::ID,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(authority, Account::new(1_000_000, 0, &Address::default()))],
    );

    assert!(
        result.program_result.is_ok(),
        "optional has_one with None should pass (constraint skipped): {:?}",
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

    // Minimum valid data: disc(1) + u32 name_len=0(4) + u32 tags_count=0(4) = 9 bytes
    let data = build_dynamic_account_data(b"", &[]);
    assert_eq!(
        data.len(),
        9,
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

fn build_mixed_account_data(authority: Address, value: u64, label: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][value(8)][u32:label_len][label_bytes]
    let label_len = label.len();
    let total = 1 + MIXED_FIXED_SIZE + 4 + label_len;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = MIXED_ACCOUNT_DISC;
    offset += 1;

    data[offset..offset + 32].copy_from_slice(authority.as_ref());
    offset += 32;

    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    offset += 8;

    data[offset..offset + 4].copy_from_slice(&(label_len as u32).to_le_bytes());
    offset += 4;

    data[offset..offset + label_len].copy_from_slice(label);

    data
}

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
    data[label_offset..label_offset + 4].copy_from_slice(&100u32.to_le_bytes());
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

#[test]
fn test_mixed_account_invalid_utf8_label() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_mixed_account_data(authority, 42, &[0xFF, 0xFE]);
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
        "invalid UTF-8 in label must be rejected"
    );
}

// ============================================================================
// SmallPrefixAccount (u8 prefix, discriminator = 7)
// ============================================================================

fn build_small_prefix_account_data(tag: &[u8], scores: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][u8:tag_len][tag_bytes][u8:scores_count][score_elements]
    let tag_len = tag.len();
    let scores_count = scores.len();
    let total = 1 + 1 + tag_len + 1 + scores_count;
    let mut data = vec![0u8; total];

    let mut offset = 0;
    data[offset] = SMALL_PREFIX_DISC;
    offset += 1;

    data[offset] = tag_len as u8;
    offset += 1;
    data[offset..offset + tag_len].copy_from_slice(tag);
    offset += tag_len;

    data[offset] = scores_count as u8;
    offset += 1;
    data[offset..offset + scores_count].copy_from_slice(scores);

    data
}

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
fn test_small_prefix_invalid_utf8_tag() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_small_prefix_account_data(&[0x80, 0x81], &[1]);
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
        "invalid UTF-8 in tag must be rejected"
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

fn build_mutate_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    new_name: &[u8],
) -> Instruction {
    // Instruction data: [disc(26)][u32:name_len][name_bytes]
    let mut data = vec![26];
    data.extend_from_slice(&(new_name.len() as u32).to_le_bytes());
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

    // Verify the account data was updated
    let result_data = &result.resulting_accounts[0].1.data;
    assert_eq!(result_data[0], DYNAMIC_ACCOUNT_DISC);
    // Read name prefix (u32 at offset 1)
    let name_len = u32::from_le_bytes(result_data[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 5);
    assert_eq!(&result_data[5..10], b"world");
    // Verify tags were preserved
    let tags_count = u32::from_le_bytes(result_data[10..14].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    assert_eq!(&result_data[14..46], tag.as_ref());
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
    let name_len = u32::from_le_bytes(result_data[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 2);
    assert_eq!(&result_data[5..7], b"hi");
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
    let name_len = u32::from_le_bytes(result_data[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 8);
    assert_eq!(&result_data[5..13], b"12345678");
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
    let name_len = u32::from_le_bytes(result_data[1..5].try_into().unwrap()) as usize;
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
    let name_len = u32::from_le_bytes(result_data[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 6);
    assert_eq!(&result_data[5..11], b"abcdef");
    // Verify tags were shifted correctly
    let tags_offset = 11;
    let tags_count = u32::from_le_bytes(
        result_data[tags_offset..tags_offset + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(tags_count, 2);
    assert_eq!(
        &result_data[tags_offset + 4..tags_offset + 36],
        tag1.as_ref()
    );
    assert_eq!(
        &result_data[tags_offset + 36..tags_offset + 68],
        tag2.as_ref()
    );
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

/// u32 prefix claiming u32::MAX bytes — validation must reject, not wrap/panic
#[test]
fn test_adversarial_prefix_u32_max_name_len() {
    let mollusk = setup();
    let account = Address::new_unique();

    let mut data = vec![0u8; 1 + 4 + 4]; // disc + name prefix + tags prefix
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&u32::MAX.to_le_bytes()); // name len = 4 billion
                                                         // tags prefix = 0 (starts right after, but name data is "missing")
    data[5..9].copy_from_slice(&0u32.to_le_bytes());

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
        "u32::MAX name prefix must be rejected (exceeds max=8)"
    );
}

/// u32 prefix just above max (9 when max=8) — off-by-one test
#[test]
fn test_adversarial_prefix_one_past_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    // Build account with 9 valid ASCII bytes but prefix says 9 (max=8)
    let mut data = vec![0u8; 1 + 4 + 9 + 4]; // disc + prefix + "aaaaaaaaa" + tags prefix
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&9u32.to_le_bytes());
    data[5..14].copy_from_slice(b"aaaaaaaaa");
    data[14..18].copy_from_slice(&0u32.to_le_bytes());

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

/// Vec prefix claiming u32::MAX element count — tests count*elem_size overflow path
#[test]
fn test_adversarial_vec_count_u32_max() {
    let mollusk = setup();
    let account = Address::new_unique();

    // DynamicAccount: name="" (prefix=0), tags count=u32::MAX
    let mut data = vec![0u8; 1 + 4 + 4]; // disc + name prefix(0) + tags prefix
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&0u32.to_le_bytes()); // empty name
    data[5..9].copy_from_slice(&u32::MAX.to_le_bytes()); // 4 billion tags

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
        "u32::MAX vec count must be rejected (exceeds max=2)"
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

    // Build with 3 tags (max=2): name="" prefix=0, tags count=3
    let mut data = vec![0u8; 1 + 4 + 4 + 32 * 3];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&0u32.to_le_bytes());
    data[5..9].copy_from_slice(&3u32.to_le_bytes());
    data[9..41].copy_from_slice(tag1.as_ref());
    data[41..73].copy_from_slice(tag2.as_ref());
    data[73..105].copy_from_slice(tag3.as_ref());

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

/// Name prefix says valid length but data crosses into tags prefix bytes.
/// Specifically: name len=8 but account only has disc(1)+prefix(4)+5 bytes.
/// The name "reads into" where tags prefix would be.
#[test]
fn test_adversarial_name_data_overlaps_tags_prefix_region() {
    let mollusk = setup();
    let account = Address::new_unique();

    // 1 + 4 + 5 = 10 bytes. Prefix says len=8 but only 5 bytes of data exist.
    let mut data = vec![0u8; 10];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&8u32.to_le_bytes()); // claims 8 bytes
    data[5..10].copy_from_slice(b"abcde"); // only 5 bytes present

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
        "prefix claiming more bytes than available must fail with AccountDataTooSmall"
    );
}

/// Tags prefix positioned correctly but data truncated: count=1 but only 16
/// of 32 tag bytes present.
#[test]
fn test_adversarial_vec_data_truncated_mid_element() {
    let mollusk = setup();
    let account = Address::new_unique();

    // name="" (prefix=0) + tags count=1 but only 16 bytes (Address is 32)
    let mut data = vec![0u8; 1 + 4 + 4 + 16];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&0u32.to_le_bytes()); // empty name
    data[5..9].copy_from_slice(&1u32.to_le_bytes()); // 1 tag
                                                     // data[9..25] = 16 zero bytes (need 32 for Address)

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
        "truncated vec element data must fail"
    );
}

// ============================================================================
// ADVERSARIAL TESTS: Multi-byte UTF-8 Edge Cases
// ============================================================================

/// Truncated 2-byte UTF-8 sequence: 0xC3 without continuation byte
#[test]
fn test_adversarial_utf8_truncated_2byte_sequence() {
    let mollusk = setup();
    let account = Address::new_unique();

    // 0xC3 starts a 2-byte UTF-8 char (e.g. é = C3 A9), but we only give 1 byte
    let data = build_dynamic_account_data(&[0xC3], &[]);
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
        "truncated 2-byte UTF-8 must be rejected"
    );
}

/// Truncated 3-byte UTF-8 sequence: euro sign is E2 82 AC, give only E2 82
#[test]
fn test_adversarial_utf8_truncated_3byte_sequence() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(&[0xE2, 0x82], &[]);
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
        "truncated 3-byte UTF-8 must be rejected"
    );
}

/// Overlong encoding: C0 80 is an overlong encoding of NUL (invalid UTF-8)
#[test]
fn test_adversarial_utf8_overlong_nul() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(&[0xC0, 0x80], &[]);
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
        "overlong UTF-8 encoding must be rejected"
    );
}

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

/// Surrogate half (ED A0 80 = U+D800) — invalid in UTF-8
#[test]
fn test_adversarial_utf8_surrogate_half() {
    let mollusk = setup();
    let account = Address::new_unique();

    let data = build_dynamic_account_data(&[0xED, 0xA0, 0x80], &[]);
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
        "UTF-8 surrogate half must be rejected"
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

    // disc + tag prefix(255) + 0 bytes of data + scores prefix
    let mut data = vec![SMALL_PREFIX_DISC, 255];
    // No actual tag data — prefix claims 255 bytes but max=100
    data.push(0); // scores count = 0

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

/// u8 scores count = 255 (max=10) — vec u8 prefix overflow
#[test]
fn test_adversarial_small_prefix_vec_u8_overflow() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + tag(prefix=0, empty) + scores(prefix=255, max=10)
    let data = vec![SMALL_PREFIX_DISC, 0, 255];

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

fn build_mutate_then_readback_instruction(
    account: Address,
    payer: Address,
    system_program: Address,
    new_name: &[u8],
    expected_tags_count: u8,
) -> Instruction {
    // Fixed args come first in ZC struct, then dynamic fields with inline prefixes.
    // Layout: [disc(27)][expected_tags_count(u8)][u32:name_len][name_bytes]
    let mut data = vec![27];
    data.push(expected_tags_count);
    data.extend_from_slice(&(new_name.len() as u32).to_le_bytes());
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"12345678", 2);
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

    // Also verify the raw bytes to be extra paranoid
    let rd = &result.resulting_accounts[0].1.data;
    assert_eq!(rd[0], DYNAMIC_ACCOUNT_DISC);
    let name_len = u32::from_le_bytes(rd[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 8);
    assert_eq!(&rd[5..13], b"12345678");
    let tags_count = u32::from_le_bytes(rd[13..17].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    assert_eq!(&rd[17..49], tag1.as_ref());
    assert_eq!(&rd[49..81], tag2.as_ref());
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"x", 2);
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
    let name_len = u32::from_le_bytes(rd[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 1);
    assert_eq!(&rd[5..6], b"x");
    let tags_count = u32::from_le_bytes(rd[6..10].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    assert_eq!(&rd[10..42], tag1.as_ref());
    assert_eq!(&rd[42..74], tag2.as_ref());
}

/// Mutate name to empty with trailing tags: edge case for zero-length memmove source
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"", 1);
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
    let name_len = u32::from_le_bytes(rd[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 0);
    let tags_count = u32::from_le_bytes(rd[5..9].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    assert_eq!(&rd[9..41], tag.as_ref());
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"12345678", 1);
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
    let name_len = u32::from_le_bytes(rd[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 8);
    assert_eq!(&rd[5..13], b"12345678");
    let tags_count = u32::from_le_bytes(rd[13..17].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 1);
    assert_eq!(&rd[17..49], tag.as_ref());
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"12345678", 2);
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"x", 2);
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"abcdef", 2);
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

    // Final byte-level verification
    let rd = &result.resulting_accounts[0].1.data;
    let name_len = u32::from_le_bytes(rd[1..5].try_into().unwrap()) as usize;
    assert_eq!(name_len, 6);
    assert_eq!(&rd[5..11], b"abcdef");
    let tags_count = u32::from_le_bytes(rd[11..15].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 2);
    assert_eq!(
        &rd[15..47],
        tag1.as_ref(),
        "tag1 corrupted after 3 mutations"
    );
    assert_eq!(
        &rd[47..79],
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
        build_mutate_then_readback_instruction(account, payer, system_program, b"hello", 1);
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

    // disc + name prefix(0) = valid empty name, but no tags prefix follows
    let mut data = vec![0u8; 5];
    data[0] = DYNAMIC_ACCOUNT_DISC;
    data[1..5].copy_from_slice(&0u32.to_le_bytes());

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

/// Partial name prefix: only 2 of 4 bytes for u32 prefix
#[test]
fn test_adversarial_partial_u32_prefix() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc + 2 bytes of what should be a 4-byte prefix
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
        "partial u32 prefix must fail"
    );
}

/// MixedAccount: valid fixed fields but label prefix extends past end
#[test]
fn test_adversarial_mixed_fixed_valid_dynamic_truncated() {
    let mollusk = setup();
    let account = Address::new_unique();

    let authority = Address::new_unique();
    // disc(1) + authority(32) + value(8) = 41 bytes of fixed data
    // Then u32 label prefix claims 10 bytes but only 2 are present
    let mut data = vec![0u8; 41 + 4 + 2]; // 47 bytes total
    data[0] = MIXED_ACCOUNT_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&42u64.to_le_bytes());
    data[41..45].copy_from_slice(&10u32.to_le_bytes()); // claims 10 bytes
    data[45..47].copy_from_slice(b"ab"); // only 2 present

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
        ProgramResult::Failure(ProgramError::AccountDataTooSmall),
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

/// All-zero account data (0-byte discriminator = potential uninitialized attack)
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

    // disc(1) + name_prefix(4, len=0) + tags_prefix(4, count=0) = 9 bytes
    let data = build_dynamic_account_data(b"", &[]);
    assert_eq!(data.len(), 9, "minimum valid account should be 9 bytes");

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

const TAIL_STR_DISC: u8 = 8;
const TAIL_BYTES_DISC: u8 = 9;
const TAIL_FIXED_SIZE: usize = 32; // Address

fn build_tail_str_account_data(authority: Address, label: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][label_bytes...]
    let mut data = vec![0u8; 1 + TAIL_FIXED_SIZE + label.len()];
    data[0] = TAIL_STR_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..].copy_from_slice(label);
    data
}

fn build_tail_bytes_account_data(authority: Address, payload: &[u8]) -> Vec<u8> {
    // Layout: [disc(1)][authority(32)][data_bytes...]
    let mut data = vec![0u8; 1 + TAIL_FIXED_SIZE + payload.len()];
    data[0] = TAIL_BYTES_DISC;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..].copy_from_slice(payload);
    data
}

fn build_tail_str_check_instruction(account: Address, expected_len: u8) -> Instruction {
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![28, expected_len],
    }
}

fn build_tail_bytes_check_instruction(account: Address, expected_len: u8) -> Instruction {
    Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![solana_instruction::AccountMeta::new_readonly(
            account, false,
        )],
        data: vec![29, expected_len],
    }
}

#[test]
fn test_tail_str_valid_utf8_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_tail_str_account_data(authority, b"hello");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid UTF-8 tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_tail_str_empty_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_tail_str_account_data(authority, b"");
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_tail_str_invalid_utf8_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_tail_str_account_data(authority, &[0xFF, 0xFE]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 2);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "invalid UTF-8 in tail str must be rejected"
    );
}

#[test]
fn test_tail_str_truncated_multibyte_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    // Truncated 3-byte UTF-8: euro sign is E2 82 AC, give only E2 82
    let data = build_tail_str_account_data(authority, &[0xE2, 0x82]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 2);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "truncated multi-byte UTF-8 in tail str must be rejected"
    );
}

#[test]
fn test_tail_str_multibyte_utf8_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    // "café" = 63 61 66 C3 A9 = 5 bytes
    let data = build_tail_str_account_data(authority, "café".as_bytes());
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid multi-byte UTF-8 tail str should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_tail_bytes_valid_data_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_tail_bytes_account_data(authority, &[0xFF, 0x00, 0xAB, 0xCD]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_bytes_check_instruction(account, 4);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "valid tail bytes should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_tail_bytes_empty_accepted() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let data = build_tail_bytes_account_data(authority, &[]);
    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_bytes_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_ok(),
        "empty tail bytes should be accepted: {:?}",
        result.program_result
    );
}

#[test]
fn test_tail_str_wrong_discriminator_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();
    let authority = Address::new_unique();

    let mut data = build_tail_str_account_data(authority, b"hello");
    data[0] = 99; // wrong discriminator

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 5);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert_eq!(
        result.program_result,
        ProgramResult::Failure(ProgramError::InvalidAccountData),
        "wrong discriminator must be rejected"
    );
}

#[test]
fn test_tail_str_truncated_fixed_section_rejected() {
    let mollusk = setup();
    let account = Address::new_unique();

    // disc(1) + only 16 bytes (need 32 for Address)
    let mut data = vec![0u8; 17];
    data[0] = TAIL_STR_DISC;

    let account_data = Account {
        lamports: 1_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction = build_tail_str_check_instruction(account, 0);
    let result = mollusk.process_instruction(&instruction, &[(account, account_data)]);

    assert!(
        result.program_result.is_err(),
        "truncated fixed section must be rejected"
    );
}
