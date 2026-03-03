use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

use quasar_test_misc::client::*;

const SIMPLE_ACCOUNT_SIZE: usize = 42; // 1 disc + 32 addr + 8 u64 + 1 u8
const MULTI_DISC_SIZE: usize = 10; // 2 disc + 8 u64

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
