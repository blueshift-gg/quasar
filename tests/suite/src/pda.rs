use {
    crate::helpers::mollusk_for_program,
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    quasar_test_pda::cpi::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
};

const CONFIG_SIZE: usize = 2;
const USER_SIZE: usize = 42;
const ITEM_SIZE: usize = 10;
const COMPLEX_SIZE: usize = 42;
const EMPTY_SEED_SIZE: usize = 2;
const MAX_SEED_SIZE: usize = 2;
const THREE_SEED_SIZE: usize = 66;

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_pda::ID, "quasar_test_pda")
}

fn build_user_account_data(authority: Address, value: u64, bump: u8) -> Vec<u8> {
    let mut data = vec![0u8; USER_SIZE];
    data[0] = 2;
    data[1..33].copy_from_slice(authority.as_ref());
    data[33..41].copy_from_slice(&value.to_le_bytes());
    data[41] = bump;
    data
}

#[test]
fn test_literal_seed_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (config, _) = Address::find_program_address(&[b"config"], &quasar_test_pda::ID);

    let instruction: Instruction = InitLiteralSeedInstruction {
        payer,
        config,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (config, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "literal seed init failed: {:?}",
        result.program_result
    );
    let config_account = &result.resulting_accounts[1].1;
    assert_eq!(config_account.data.len(), CONFIG_SIZE);
    assert_eq!(config_account.data[0], 1);
    assert_ne!(config_account.data[1], 0);
    assert_eq!(config_account.owner, quasar_test_pda::ID);
}

#[test]
fn test_pubkey_seed_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (user, _) = Address::find_program_address(&[b"user", payer.as_ref()], &quasar_test_pda::ID);

    let instruction: Instruction = InitPubkeySeedInstruction {
        payer,
        user,
        system_program,
        value: 42,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (user, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "pubkey seed init failed: {:?}",
        result.program_result
    );
    let user_account = &result.resulting_accounts[1].1;
    assert_eq!(user_account.data.len(), USER_SIZE);
    assert_eq!(user_account.data[0], 2);
    assert_eq!(&user_account.data[1..33], payer.as_ref());
    assert_eq!(
        u64::from_le_bytes(user_account.data[33..41].try_into().unwrap()),
        42
    );
    assert_eq!(user_account.owner, quasar_test_pda::ID);
}

#[test]
fn test_instruction_seed_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let authority = Address::new_unique();
    let (item, _) =
        Address::find_program_address(&[b"item", authority.as_ref()], &quasar_test_pda::ID);

    let instruction: Instruction = InitInstructionSeedInstruction {
        payer,
        authority,
        item,
        system_program,
        id: 123,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (authority, Account::new(1_000_000, 0, &system_program)),
            (item, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "instruction seed init failed: {:?}",
        result.program_result
    );
    let item_account = &result.resulting_accounts[2].1;
    assert_eq!(item_account.data.len(), ITEM_SIZE);
    assert_eq!(item_account.data[0], 3);
    assert_eq!(
        u64::from_le_bytes(item_account.data[1..9].try_into().unwrap()),
        123
    );
    assert_eq!(item_account.owner, quasar_test_pda::ID);
}

#[test]
fn test_multi_seed_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let authority = Address::new_unique();
    let (complex, _) = Address::find_program_address(
        &[b"complex", payer.as_ref(), authority.as_ref()],
        &quasar_test_pda::ID,
    );

    let instruction: Instruction = InitMultiSeedsInstruction {
        payer,
        authority,
        complex,
        system_program,
        amount: 500,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (authority, Account::new(1_000_000, 0, &system_program)),
            (complex, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "multi seed init failed: {:?}",
        result.program_result
    );
    let complex_account = &result.resulting_accounts[2].1;
    assert_eq!(complex_account.data.len(), COMPLEX_SIZE);
    assert_eq!(complex_account.data[0], 4);
    assert_eq!(&complex_account.data[1..33], authority.as_ref());
    assert_eq!(
        u64::from_le_bytes(complex_account.data[33..41].try_into().unwrap()),
        500
    );
    assert_eq!(complex_account.owner, quasar_test_pda::ID);
}

#[test]
fn test_wrong_seeds_fail() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let wrong_pda = Address::new_unique();

    let instruction: Instruction = InitLiteralSeedInstruction {
        payer,
        config: wrong_pda,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (wrong_pda, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong PDA address"
    );
}

#[test]
fn test_wrong_bump_fail() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, correct_bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let wrong_bump = if correct_bump == 0 {
        1
    } else {
        correct_bump - 1
    };
    let account_data = build_user_account_data(authority, 42, wrong_bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong bump stored in account"
    );
}

#[test]
fn test_update_pda_success() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "update PDA failed: {:?}",
        result.program_result
    );
    let updated = &result.resulting_accounts[1].1;
    assert_eq!(
        u64::from_le_bytes(updated.data[33..41].try_into().unwrap()),
        100
    );
}

#[test]
fn test_update_pda_wrong_seeds() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_pda = Address::new_unique();
    let (_, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: wrong_pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                wrong_pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong PDA address for update"
    );
}

#[test]
fn test_update_pda_wrong_authority() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority: wrong_authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                wrong_authority,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong authority"
    );
}

#[test]
fn test_close_pda() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let pda_lamports: u64 = 1_000_000;
    let authority_lamports: u64 = 500_000;
    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = ClosePdaInstruction {
        authority,
        user: pda,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                authority,
                Account::new(authority_lamports, 0, &Address::default()),
            ),
            (
                pda,
                Account {
                    lamports: pda_lamports,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "close PDA failed: {:?}",
        result.program_result
    );
    let closed = &result.resulting_accounts[1].1;
    assert_eq!(closed.lamports, 0);
    assert!(closed.data.iter().all(|&b| b == 0));
    let authority_after = &result.resulting_accounts[0].1;
    assert_eq!(authority_after.lamports, authority_lamports + pda_lamports);
}

#[test]
fn test_pda_signer_transfer() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);
    let recipient = Address::new_unique();

    let pda_lamports: u64 = 10_000_000;
    let transfer_amount: u64 = 1_000_000;
    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = PdaTransferInstruction {
        authority,
        pda,
        recipient,
        amount: transfer_amount,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: pda_lamports,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (recipient, Account::new(500_000, 0, &Address::default())),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "PDA transfer failed: {:?}",
        result.program_result
    );
    let pda_after = &result.resulting_accounts[1].1;
    assert_eq!(pda_after.lamports, pda_lamports - transfer_amount);
    let recipient_after = &result.resulting_accounts[2].1;
    assert_eq!(recipient_after.lamports, 500_000 + transfer_amount);
}

#[test]
fn test_pda_signer_wrong_seeds() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);
    let recipient = Address::new_unique();

    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = PdaTransferInstruction {
        authority: wrong_authority,
        pda,
        recipient,
        amount: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                wrong_authority,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                pda,
                Account {
                    lamports: 10_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (recipient, Account::new(500_000, 0, &Address::default())),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong authority for PDA transfer"
    );
}

#[test]
fn test_pda_bump_from_account() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (pda, expected_bump) =
        Address::find_program_address(&[b"user", payer.as_ref()], &quasar_test_pda::ID);

    let instruction: Instruction = InitPubkeySeedInstruction {
        payer,
        user: pda,
        system_program,
        value: 99,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (pda, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(result.program_result.is_ok());
    let user_account = &result.resulting_accounts[1].1;
    let stored_bump = user_account.data[41];
    assert_eq!(
        stored_bump, expected_bump,
        "Stored bump {} != expected bump {}",
        stored_bump, expected_bump
    );
}

#[test]
fn test_pda_cu() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (config, _) = Address::find_program_address(&[b"config"], &quasar_test_pda::ID);

    let instruction: Instruction = InitLiteralSeedInstruction {
        payer,
        config,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (config, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(result.program_result.is_ok());
    println!(
        "PDA init (literal seed) CU: {}",
        result.compute_units_consumed
    );
}

// ── Seed type tests ─────────────────────────────────────────────────────

#[test]
fn test_empty_seed() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (empty, _) = Address::find_program_address(&[b""], &quasar_test_pda::ID);

    let instruction: Instruction = InitEmptySeedInstruction {
        payer,
        empty,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (empty, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "empty seed init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[1].1;
    assert_eq!(account.data.len(), EMPTY_SEED_SIZE);
    assert_eq!(account.data[0], 5);
    assert_eq!(account.owner, quasar_test_pda::ID);
}

#[test]
fn test_max_seed_length() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (max_seed, _) =
        Address::find_program_address(&[b"abcdefghijklmnopqrstuvwxyz012345"], &quasar_test_pda::ID);

    let instruction: Instruction = InitMaxSeedLengthInstruction {
        payer,
        max_seed,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (max_seed, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "max seed init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[1].1;
    assert_eq!(account.data.len(), MAX_SEED_SIZE);
    assert_eq!(account.data[0], 6);
    assert_eq!(account.owner, quasar_test_pda::ID);
}

#[test]
fn test_seed_with_special_chars() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let special_key = Address::new_unique();
    let (item, _) =
        Address::find_program_address(&[b"item", special_key.as_ref()], &quasar_test_pda::ID);

    let instruction: Instruction = InitInstructionSeedInstruction {
        payer,
        authority: special_key,
        item,
        system_program,
        id: 0xFF_FF_FF_FF_FF_FF_FF_FF,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (special_key, Account::new(1_000_000, 0, &system_program)),
            (item, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "special chars seed init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[2].1;
    assert_eq!(account.data.len(), ITEM_SIZE);
    assert_eq!(account.owner, quasar_test_pda::ID);
}

// ── Error path tests ────────────────────────────────────────────────────

#[test]
fn test_wrong_seeds_different_literal() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (wrong_pda, _) = Address::find_program_address(&[b"not-config"], &quasar_test_pda::ID);

    let instruction: Instruction = InitLiteralSeedInstruction {
        payer,
        config: wrong_pda,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (wrong_pda, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with different literal seed PDA"
    );
}

#[test]
fn test_wrong_bump_off_by_one() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, correct_bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let wrong_bump = correct_bump.wrapping_add(1);
    let account_data = build_user_account_data(authority, 42, wrong_bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with bump+1"
    );
}

#[test]
fn test_wrong_bump_zero() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, correct_bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let account_data = build_user_account_data(authority, 42, 0);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    if correct_bump != 0 {
        assert!(
            result.program_result.is_err(),
            "Expected failure with bump=0 when real bump is {}",
            correct_bump
        );
    }
}

#[test]
fn test_pda_account_wrong_owner() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);

    let wrong_owner = Address::new_unique();
    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = UpdatePdaInstruction {
        authority,
        user: pda,
        new_value: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: 1_000_000,
                    data: account_data,
                    owner: wrong_owner,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong owner"
    );
}

#[test]
fn test_pda_account_not_writable_on_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let (config, _) = Address::find_program_address(&[b"config"], &quasar_test_pda::ID);

    let mut instruction: Instruction = InitLiteralSeedInstruction {
        payer,
        config,
        system_program,
    }
    .into();

    instruction.accounts[1].is_writable = false;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (config, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure when PDA not writable for init"
    );
}

// ── Multi-seed tests ────────────────────────────────────────────────────

#[test]
fn test_multi_seed_three_components() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let first = Address::new_unique();
    let second = Address::new_unique();
    let (triple, _) = Address::find_program_address(
        &[b"triple", first.as_ref(), second.as_ref()],
        &quasar_test_pda::ID,
    );

    let instruction: Instruction = InitThreeSeedsInstruction {
        payer,
        first,
        second,
        triple,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (first, Account::new(1_000_000, 0, &system_program)),
            (second, Account::new(1_000_000, 0, &system_program)),
            (triple, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "three seed init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[3].1;
    assert_eq!(account.data.len(), THREE_SEED_SIZE);
    assert_eq!(account.data[0], 7);
    assert_eq!(&account.data[1..33], first.as_ref());
    assert_eq!(&account.data[33..65], second.as_ref());
    assert_eq!(account.owner, quasar_test_pda::ID);
}

#[test]
fn test_max_multi_seeds() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let authority = Address::new_unique();
    let max_seeds: Vec<&[u8]> = (0..15).map(|_| b"max".as_slice()).collect();
    let (complex, _) = Address::find_program_address(&max_seeds, &quasar_test_pda::ID);

    let instruction: Instruction = InitMaxMultiSeedsInstruction {
        payer,
        authority,
        complex,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (authority, Account::new(1_000_000, 0, &system_program)),
            (complex, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "MAX 15 seeds verification failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_multi_seed_with_address_and_literal() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let authority = Address::new_unique();
    let (complex, _) = Address::find_program_address(
        &[b"complex", payer.as_ref(), authority.as_ref()],
        &quasar_test_pda::ID,
    );

    let instruction: Instruction = InitMultiSeedsInstruction {
        payer,
        authority,
        complex,
        system_program,
        amount: 12345,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (authority, Account::new(1_000_000, 0, &system_program)),
            (complex, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "address+literal seed init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[2].1;
    assert_eq!(account.data[0], 4);
    assert_eq!(&account.data[1..33], authority.as_ref());
    assert_eq!(
        u64::from_le_bytes(account.data[33..41].try_into().unwrap()),
        12345
    );
}

#[test]
fn test_multi_seed_order_matters() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let first = Address::new_unique();
    let second = Address::new_unique();

    let (triple_swapped, _) = Address::find_program_address(
        &[b"triple", second.as_ref(), first.as_ref()],
        &quasar_test_pda::ID,
    );

    let instruction: Instruction = InitThreeSeedsInstruction {
        payer,
        first,
        second,
        triple: triple_swapped,
        system_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (first, Account::new(1_000_000, 0, &system_program)),
            (second, Account::new(1_000_000, 0, &system_program)),
            (triple_swapped, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure when seed order is swapped"
    );
}

// ── PDA signer tests ────────────────────────────────────────────────────

#[test]
fn test_pda_signer_cpi_success() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);
    let recipient = Address::new_unique();

    let pda_lamports: u64 = 10_000_000;
    let transfer_amount: u64 = 500_000;
    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = PdaTransferInstruction {
        authority,
        pda,
        recipient,
        amount: transfer_amount,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (
                pda,
                Account {
                    lamports: pda_lamports,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (recipient, Account::new(0, 0, &Address::default())),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "PDA signer CPI transfer failed: {:?}",
        result.program_result
    );
    let recipient_after = &result.resulting_accounts[2].1;
    assert_eq!(recipient_after.lamports, transfer_amount);
}

#[test]
fn test_pda_signer_wrong_authority_cpi() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let (pda, bump) =
        Address::find_program_address(&[b"user", authority.as_ref()], &quasar_test_pda::ID);
    let recipient = Address::new_unique();

    let account_data = build_user_account_data(authority, 42, bump);

    let instruction: Instruction = PdaTransferInstruction {
        authority: wrong_authority,
        pda,
        recipient,
        amount: 100,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (
                wrong_authority,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                pda,
                Account {
                    lamports: 10_000_000,
                    data: account_data,
                    owner: quasar_test_pda::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            ),
            (recipient, Account::new(0, 0, &Address::default())),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong authority for PDA signer CPI"
    );
}

// ── Typed seed with instruction data ────────────────────────────────────

const INDEXED_SIZE: usize = 42;

#[test]
fn test_typed_seed_ix_data_init() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let authority = Address::new_unique();
    let index: u64 = 42;

    let (item_pda, _) = Address::find_program_address(
        &[b"indexed", authority.as_ref(), &index.to_le_bytes()],
        &quasar_test_pda::ID,
    );

    let instruction: Instruction = InitIxDataSeedInstruction {
        payer,
        authority,
        item: item_pda,
        system_program,
        index,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (authority, Account::new(1_000_000, 0, &system_program)),
            (item_pda, Account::default()),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "typed seed with ix data init failed: {:?}",
        result.program_result
    );
    let account = &result.resulting_accounts[2].1;
    assert_eq!(account.data.len(), INDEXED_SIZE);
    assert_eq!(account.data[0], 8); // discriminator
    assert_eq!(&account.data[1..33], authority.as_ref());
    assert_eq!(
        u64::from_le_bytes(account.data[33..41].try_into().unwrap()),
        42
    );
    assert_eq!(account.owner, quasar_test_pda::ID);
}

// ── Typed seed with deserialized account field ──────────────────────────

const NS_CONFIG_SIZE: usize = 6;
const SCOPED_ITEM_SIZE: usize = 14;

#[test]
fn test_typed_seed_deserialized_field() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let namespace: u32 = 42;

    // Step 1: Create the NamespaceConfig
    let (config_pda, _) = Address::find_program_address(&[b"ns_config"], &quasar_test_pda::ID);

    let init_config_ix: Instruction = InitNsConfigInstruction {
        payer,
        config: config_pda,
        system_program,
        namespace,
    }
    .into();

    let result = mollusk.process_instruction(
        &init_config_ix,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (config_pda, Account::default()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "init ns_config failed: {:?}",
        result.program_result
    );
    let config_account = result.resulting_accounts[1].1.clone();
    assert_eq!(config_account.data.len(), NS_CONFIG_SIZE);
    assert_eq!(config_account.data[0], 9); // discriminator

    // Step 2: Create the ScopedItem using namespace as instruction arg seed
    let (item_pda, _) =
        Address::find_program_address(&[b"scoped", &namespace.to_le_bytes()], &quasar_test_pda::ID);

    let init_item_ix: Instruction = InitScopedItemInstruction {
        payer,
        item: item_pda,
        system_program,
        namespace,
    }
    .into();

    let payer_after_config = result.resulting_accounts[0].1.clone();
    let result2 = mollusk.process_instruction(
        &init_item_ix,
        &[
            (payer, payer_after_config),
            (item_pda, Account::default()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result2.program_result.is_ok(),
        "init scoped_item failed: {:?}",
        result2.program_result
    );
    let item_account = result2.resulting_accounts[1].1.clone();
    assert_eq!(item_account.data.len(), SCOPED_ITEM_SIZE);
    assert_eq!(item_account.data[0], 10); // discriminator

    // Step 3: Verify the ScopedItem using config.namespace as seed
    let verify_ix: Instruction = VerifyScopedItemInstruction {
        config: config_pda,
        item: item_pda,
    }
    .into();

    let result3 = mollusk.process_instruction(
        &verify_ix,
        &[(config_pda, config_account), (item_pda, item_account)],
    );
    assert!(
        result3.program_result.is_ok(),
        "verify scoped_item with deserialized field seed failed: {:?}",
        result3.program_result
    );
}

// ── Init with account field seed ─────────────────────────────────────────────

#[test]
fn test_init_typed_seed_from_account_field() {
    // Verifies that `seeds = ScopedItem::seeds(config.namespace)` works when
    // `item` is an `init` account — i.e. `typed_seed_slice_expr_init` correctly
    // casts `&mut AccountView` → `&NamespaceConfig` to access `.namespace`.
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let namespace: u32 = 99;

    // Step 1: Create the NamespaceConfig
    let (config_pda, _) = Address::find_program_address(&[b"ns_config"], &quasar_test_pda::ID);

    let init_config_ix: Instruction = InitNsConfigInstruction {
        payer,
        config: config_pda,
        system_program,
        namespace,
    }
    .into();

    let result = mollusk.process_instruction(
        &init_config_ix,
        &[
            (payer, Account::new(1_000_000_000, 0, &system_program)),
            (config_pda, Account::default()),
            (system_program, system_program_account.clone()),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "init ns_config failed: {:?}",
        result.program_result
    );
    let config_account = result.resulting_accounts[1].1.clone();

    // Step 2: Init the ScopedItem using config.namespace (field access seed)
    let (item_pda, _) =
        Address::find_program_address(&[b"scoped", &namespace.to_le_bytes()], &quasar_test_pda::ID);

    let init_from_config_ix: Instruction = InitScopedItemFromConfigInstruction {
        payer,
        config: config_pda,
        item: item_pda,
        system_program,
    }
    .into();

    let payer_after_config = result.resulting_accounts[0].1.clone();
    let result2 = mollusk.process_instruction(
        &init_from_config_ix,
        &[
            (payer, payer_after_config),
            (config_pda, config_account),
            (item_pda, Account::default()),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result2.program_result.is_ok(),
        "init scoped_item from config field seed failed: {:?}",
        result2.program_result
    );
    let item_account = &result2.resulting_accounts[2].1;
    assert_eq!(item_account.data.len(), SCOPED_ITEM_SIZE);
    assert_eq!(item_account.data[0], 10); // discriminator
                                          // namespace field is at bytes 1..5 (u32 le)
    assert_eq!(&item_account.data[1..5], &namespace.to_le_bytes());
}
