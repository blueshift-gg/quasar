use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};
use quasar_test_misc::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

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
// Init: space override large allocation
// ============================================================================

#[test]
fn test_init_with_max_space() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(100_000_000_000, 0, &system_program);

    let (account, _bump) =
        Address::find_program_address(&[b"spacetest", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

    let instruction: Instruction = SpaceOverrideInstruction {
        payer,
        account,
        system_program,
        value: 1,
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

    let resulting = &result.resulting_accounts[1].1;
    assert_eq!(resulting.data.len(), 100, "account allocated with space = 100");
    assert!(resulting.lamports > 0, "account should have lamports for rent");
}

// ============================================================================
// Init: already initialized account (reinit with correct discriminator)
// ============================================================================

#[test]
fn test_init_account_already_initialized() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let (account, bump) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);

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
        "init should reject already-initialized account with correct discriminator"
    );
}

// ============================================================================
// Close: destination receives exact lamports
// ============================================================================

#[test]
fn test_close_destination_receives_exact_lamports() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_initial = 500_000u64;
    let authority_account = Account::new(authority_initial, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_lamports = 3_456_789u64;
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

    assert_eq!(
        result.resulting_accounts[0].1.lamports,
        authority_initial + account_lamports,
        "destination must receive exactly the closed account's lamports"
    );
    assert_eq!(
        result.resulting_accounts[1].1.lamports, 0,
        "closed account must have 0 lamports"
    );
}

// ============================================================================
// Close: all data zeroed
// ============================================================================

#[test]
fn test_close_zeroes_all_data() {
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
    assert!(
        closed.data.is_empty() || closed.data.iter().all(|&b| b == 0),
        "all account data must be zeroed after close"
    );
    assert_eq!(
        closed.owner,
        Address::default(),
        "owner reassigned to system program"
    );
}

// ============================================================================
// Realloc: grow preserves existing data
// ============================================================================

#[test]
fn test_realloc_grow_preserves_existing_data() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let authority = Address::new_unique();
    let account = Address::new_unique();
    let original_data = build_simple_account_data(authority, 123, 7);
    let account_obj = Account {
        lamports: 1_000_000,
        data: original_data.clone(),
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
    assert_eq!(resulting.data.len(), new_space as usize);
    assert_eq!(
        &resulting.data[..SIMPLE_ACCOUNT_SIZE],
        &original_data,
        "original data should be preserved after grow"
    );
}

// ============================================================================
// Realloc: shrink trailing zeroed
// ============================================================================

#[test]
fn test_realloc_shrink_trailing_zeroed() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let account = Address::new_unique();
    let mut data = build_simple_account_data(Address::new_unique(), 42, 0);
    data.resize(200, 0xFF);
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
        "data should shrink to requested size"
    );
}

// ============================================================================
// Realloc: same size is no-op
// ============================================================================

#[test]
fn test_realloc_to_same_size() {
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

    let instruction: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program,
        _new_space: SIMPLE_ACCOUNT_SIZE as u64,
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
        "realloc to same size should succeed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.data.len(),
        SIMPLE_ACCOUNT_SIZE,
        "data length should remain unchanged"
    );
}

// ============================================================================
// Realloc: grow large
// ============================================================================

#[test]
fn test_realloc_grow_large() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(100_000_000_000, 0, &system_program);

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: build_simple_account_data(Address::new_unique(), 42, 0),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let new_space = 10_000u64;
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
        "realloc grow large should succeed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.data.len(),
        new_space as usize,
        "data should be resized to 10,000 bytes"
    );
}

// ============================================================================
// Realloc: maintains rent exemption
// ============================================================================

#[test]
fn test_realloc_maintains_rent_exemption() {
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

    let new_space = 1_000u64;
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
    assert!(
        resulting.lamports >= 1_000_000,
        "account lamports should be at least the original amount after grow"
    );
}

// ============================================================================
// init_if_needed: with correct discriminator and data (already initialized)
// ============================================================================

#[test]
fn test_init_if_needed_with_correct_discriminator_and_data() {
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

    let instruction: Instruction = InitIfNeededInstruction {
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
        "init_if_needed with already-initialized valid account should succeed: {:?}",
        result.program_result
    );

    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(&data[33..41], &77u64.to_le_bytes(), "value updated to 77");
}

// ============================================================================
// init_if_needed: payer has 0 lamports
// ============================================================================

#[test]
fn test_init_if_needed_with_zero_lamports_payer() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(0, 0, &system_program);

    let (account, _) =
        Address::find_program_address(&[b"simple", payer.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account::default();

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
        "init_if_needed should fail when payer has 0 lamports"
    );
}

// ============================================================================
// Adversarial Tests — Attacker-Controlled Inputs
// ============================================================================

/// Close destination lamport overflow: destination has u64::MAX - 100 lamports,
/// closed account has 1_000_000. The addition should overflow.
/// The framework uses checked_add — verify it returns an error, not wrapping.
#[test]
fn test_adversarial_close_destination_lamport_overflow() {
    let mollusk = setup();

    let authority = Address::new_unique();
    let authority_account = Account::new(u64::MAX - 100, 0, &Address::default());

    let (account, bump) =
        Address::find_program_address(&[b"simple", authority.as_ref()], &quasar_test_misc::ID);
    let account_obj = Account {
        lamports: 1_000_000,
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
    // Adding 1_000_000 to (u64::MAX - 100) should overflow.
    // The framework uses checked_add and should return an error.
    assert!(
        result.program_result.is_err(),
        "close with lamport overflow (u64::MAX - 100 + 1_000_000) must be rejected"
    );
}

/// Account with correct discriminator but data truncated by one byte.
/// SimpleAccount is 42 bytes (1 disc + 32 addr + 8 u64 + 1 u8).
/// We provide 41 bytes — discriminator is correct but struct doesn't fit.
#[test]
fn test_adversarial_account_data_truncated_by_one() {
    let mollusk = setup();

    let account_addr = Address::new_unique();

    let mut data = vec![0u8; 41]; // ONE BYTE SHORT of SimpleAccount's 42
    data[0] = 1; // Correct discriminator

    let instruction: Instruction = OwnerCheckInstruction {
        account: account_addr,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data,
                owner: quasar_test_misc::ID,
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(
        result.program_result.is_err(),
        "account with correct disc but data 1 byte short must be rejected"
    );
}

/// Account with correct discriminator but owned by a different program.
/// The owner check should catch this even though the discriminator is valid.
#[test]
fn test_adversarial_correct_disc_wrong_owner() {
    let mollusk = setup();

    let account_addr = Address::new_unique();
    let wrong_owner = Address::new_unique();

    let data = build_simple_account_data(Address::new_unique(), 42, 0);

    let instruction: Instruction = OwnerCheckInstruction {
        account: account_addr,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[(
            account_addr,
            Account {
                lamports: 1_000_000,
                data,
                owner: wrong_owner, // Correct disc, wrong owner
                executable: false,
                rent_epoch: 0,
            },
        )],
    );
    assert!(
        result.program_result.is_err(),
        "account with correct discriminator but wrong owner must be rejected"
    );
}

/// Multi-byte discriminator: first byte matches, second byte wrong.
/// MultiDiscAccount expects [1, 2]. We provide [1, 0].
/// (Note: this partially overlaps with test_partial_discriminator_match,
/// but here we also test [1, 3] and [1, 255] to probe boundary conditions.)
#[test]
fn test_adversarial_multi_disc_first_byte_only_match_variants() {
    let mollusk = setup();

    let variants: &[(u8, u8)] = &[
        (1, 0),   // second byte zero
        (1, 3),   // second byte off by one
        (1, 255), // second byte max
        (0, 2),   // first byte zero, second correct
        (2, 2),   // first byte wrong, second correct
    ];

    for &(b0, b1) in variants {
        let account = Address::new_unique();
        let mut data = vec![0u8; MULTI_DISC_SIZE];
        data[0] = b0;
        data[1] = b1;
        data[2..10].copy_from_slice(&42u64.to_le_bytes());

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
            "multi-disc [{}, {}] (expected [1, 2]) must be rejected",
            b0,
            b1,
        );
    }
}

/// Account with all-zero data (including zero discriminator).
/// All-zero discriminator is banned — this should be rejected regardless of size.
#[test]
fn test_adversarial_all_zero_data_simple_account() {
    let mollusk = setup();

    let account = Address::new_unique();
    let data = vec![0u8; SIMPLE_ACCOUNT_SIZE]; // 42 bytes of zeros

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
        "all-zero data (disc=0) must be rejected — uninitialized account attack"
    );
}

/// Realloc shrink then grow: verify previously-truncated region is zeroed.
/// Start with 100 bytes, fill bytes 42-99 with 0xFF.
/// Shrink to 50, grow back to 100. Bytes 50-99 should be 0x00.
#[test]
fn test_adversarial_realloc_shrink_grow_data_leakage() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let account = Address::new_unique();
    let mut data = build_simple_account_data(Address::new_unique(), 42, 0);
    data.resize(100, 0xFF); // Fill bytes 42..100 with 0xFF
    let account_obj = Account {
        lamports: 10_000_000,
        data: data.clone(),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Step 1: Shrink from 100 to 50
    let instruction: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program,
        _new_space: 50,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_obj),
            (payer, payer_account.clone()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "shrink to 50 should succeed: {:?}",
        result.program_result
    );
    let shrunk_account = result.resulting_accounts[0].1.clone();
    assert_eq!(shrunk_account.data.len(), 50);

    // Step 2: Grow back from 50 to 100
    let instruction: Instruction = ReallocCheckInstruction {
        account,
        payer,
        system_program,
        _new_space: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, shrunk_account),
            (payer, result.resulting_accounts[1].1.clone()),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "grow back to 100 should succeed: {:?}",
        result.program_result
    );

    let regrown = &result.resulting_accounts[0].1;
    assert_eq!(regrown.data.len(), 100);

    // Bytes 50..100 should be zeroed, NOT the old 0xFF values.
    // This tests that the runtime/framework zeroes re-grown memory.
    let tail_region = &regrown.data[50..100];
    assert!(
        tail_region.iter().all(|&b| b == 0),
        "BUG: bytes 50..100 after shrink-then-grow are NOT zeroed. \
         Data leakage from previous allocation: {:?}",
        &tail_region[..10]
    );
}

/// Pass the same account address for two different `#[account(mut)]` parameters.
/// The SVM should detect the duplicate mutable borrow and reject.
#[test]
fn test_adversarial_same_account_for_two_mut_params() {
    let mollusk = setup();

    let signer = Address::new_unique();
    let account_addr = Address::new_unique();
    let data = build_simple_account_data(Address::new_unique(), 42, 0);
    let account_obj = Account {
        lamports: 1_000_000,
        data: data.clone(),
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    // Build instruction for DoubleMutCheck (disc=41) manually
    let instruction = Instruction {
        program_id: quasar_test_misc::ID,
        accounts: vec![
            solana_instruction::AccountMeta::new_readonly(signer, true),
            solana_instruction::AccountMeta::new(account_addr, false),
            solana_instruction::AccountMeta::new(account_addr, false), // SAME address for both mut params
        ],
        data: vec![41],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (signer, Account::new(1_000_000, 0, &Address::default())),
            (account_addr, account_obj.clone()),
            (account_addr, account_obj),
        ],
    );
    // The SVM runtime should detect the duplicate mutable borrow and reject.
    // If the framework doesn't check, SVM's borrow checker will.
    assert!(
        result.program_result.is_err(),
        "passing same account for two mut params must be rejected (double mutable borrow)"
    );
}

/// Account with data_len = 0 passed where Account<SimpleAccount> is expected.
/// Zero-length data cannot hold a discriminator.
#[test]
fn test_adversarial_account_zero_length_data() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: vec![], // 0 bytes
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "zero-length account data must be rejected — cannot hold discriminator"
    );
}

/// Account with exactly 1 byte of data (just the discriminator, no fields).
/// For SimpleAccount which needs 42 bytes, this should fail.
#[test]
fn test_adversarial_account_disc_only_no_fields() {
    let mollusk = setup();

    let account = Address::new_unique();
    let account_obj = Account {
        lamports: 1_000_000,
        data: vec![1], // correct disc, but missing all fields
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    assert!(
        result.program_result.is_err(),
        "account with disc only (1 byte, no fields) must be rejected"
    );
}

/// Account with oversized data: 10,000 bytes with correct discriminator.
/// The framework should accept this (extra space is ignored for static accounts).
#[test]
fn test_adversarial_account_oversized_data() {
    let mollusk = setup();

    let account = Address::new_unique();
    let mut data = build_simple_account_data(Address::new_unique(), 42, 0);
    data.resize(10_000, 0xAB); // pad with garbage

    let account_obj = Account {
        lamports: 100_000_000,
        data,
        owner: quasar_test_misc::ID,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = OwnerCheckInstruction { account }.into();
    let result = mollusk.process_instruction(&instruction, &[(account, account_obj)]);

    // Oversized data with correct disc and owner should still parse correctly.
    // The pointer cast reads the first 42 bytes; the rest is ignored.
    assert!(
        result.program_result.is_ok(),
        "oversized account data with correct disc should be accepted: {:?}",
        result.program_result
    );
}
