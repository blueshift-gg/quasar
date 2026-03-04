extern crate std;

use alloc::vec;
use alloc::vec::Vec;
use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use solana_account::Account;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

use crate::client::{
    CreateInstruction, DepositInstruction, ExecuteTransferInstruction, SetLabelInstruction,
};

fn setup() -> Mollusk {
    Mollusk::new(&crate::ID, "../../target/deploy/quasar_multisig")
}

fn build_config_data(
    creator: Address,
    threshold: u8,
    bump: u8,
    label: &str,
    signers: &[Address],
) -> Vec<u8> {
    build_config_data_bytes(creator, threshold, bump, label.as_bytes(), signers)
}

fn build_config_data_bytes(
    creator: Address,
    threshold: u8,
    bump: u8,
    label: &[u8],
    signers: &[Address],
) -> Vec<u8> {
    let zc_header_size = 32 + 1 + 1 + 2 + 2; // creator + threshold + bump + label_end + signers_end
    let total = 1 + zc_header_size + label.len() + signers.len() * 32;
    let mut data = vec![0u8; total];

    // Discriminator
    data[0] = 1;

    // ZC header starts at offset 1
    let header = &mut data[1..];

    // creator (32 bytes)
    header[..32].copy_from_slice(creator.as_ref());
    // threshold (1 byte)
    header[32] = threshold;
    // bump (1 byte)
    header[33] = bump;
    // label_end (cumulative byte offset: label bytes)
    let label_end = label.len() as u16;
    header[34..36].copy_from_slice(&label_end.to_le_bytes());
    // signers_end (cumulative byte offset: label bytes + signer bytes)
    let signers_end = label_end + (signers.len() as u16 * 32);
    header[36..38].copy_from_slice(&signers_end.to_le_bytes());

    // Variable tail: label bytes, then signer addresses
    let tail_start = 1 + zc_header_size;
    data[tail_start..tail_start + label.len()].copy_from_slice(label);
    let signers_start = tail_start + label.len();
    for (i, signer) in signers.iter().enumerate() {
        data[signers_start + i * 32..signers_start + (i + 1) * 32].copy_from_slice(signer.as_ref());
    }

    data
}

#[test]
fn test_create() {
    let mollusk = setup();

    let (system_program, system_program_account) = keyed_account_for_system_program();
    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let creator = Address::new_unique();
    let creator_account = Account::new(10_000_000_000, 0, &system_program);

    let (config, _config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_account = Account::default();

    let signer1 = Address::new_unique();
    let signer1_account = Account::default();
    let signer2 = Address::new_unique();
    let signer2_account = Account::default();
    let signer3 = Address::new_unique();
    let signer3_account = Account::default();

    let threshold: u8 = 2;

    // Build instruction with remaining accounts for signers
    let mut instruction: Instruction = CreateInstruction {
        creator,
        config,
        rent,
        system_program,
        threshold,
    }
    .into();

    // Add remaining accounts (signers)
    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer1, true));
    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer2, true));
    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer3, true));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (creator, creator_account),
            (config, config_account),
            (rent, rent_account),
            (system_program, system_program_account),
            (signer1, signer1_account),
            (signer2, signer2_account),
            (signer3, signer3_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "create failed: {:?}",
        result.program_result
    );

    // Verify config account data
    let config_data = &result.resulting_accounts[1].1.data;
    assert_eq!(config_data[0], 1, "discriminator should be 1");

    // Verify threshold (offset: disc(1) + creator(32) = 33)
    assert_eq!(config_data[33], threshold, "threshold mismatch");

    // Verify signers_end (offset: disc(1) + creator(32) + threshold(1) + bump(1) + label_end(2) = 37)
    let signers_end = u16::from_le_bytes([config_data[37], config_data[38]]);
    assert_eq!(
        signers_end,
        3 * 32,
        "signers_end should be 96 bytes (3 addresses)"
    );

    std::println!("\n========================================");
    std::println!("  CREATE CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_deposit() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let creator = Address::new_unique();
    let signer1 = Address::new_unique();
    let signer2 = Address::new_unique();

    let (config, config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_data = build_config_data(creator, 2, config_bump, "", &[signer1, signer2]);
    let config_account = Account {
        lamports: 1_000_000,
        data: config_data,
        owner: crate::ID,
        executable: false,
        rent_epoch: 0,
    };

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", config.as_ref()], &crate::ID);
    let vault_account = Account::new(0, 0, &system_program);

    let depositor = Address::new_unique();
    let depositor_account = Account::new(10_000_000_000, 0, &system_program);

    let deposit_amount: u64 = 1_000_000_000;

    let instruction: Instruction = DepositInstruction {
        depositor,
        config,
        vault,
        system_program,
        amount: deposit_amount,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (depositor, depositor_account),
            (config, config_account),
            (vault, vault_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "deposit failed: {:?}",
        result.program_result
    );

    let vault_after = result.resulting_accounts[2].1.lamports;
    assert_eq!(vault_after, deposit_amount, "vault lamports after deposit");

    std::println!("\n========================================");
    std::println!("  DEPOSIT CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_set_label() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let creator = Address::new_unique();
    let creator_account = Account::new(10_000_000_000, 0, &system_program);

    let signer1 = Address::new_unique();

    let (config, config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_data = build_config_data(creator, 1, config_bump, "", &[signer1]);
    let config_account = Account {
        lamports: 1_000_000,
        data: config_data,
        owner: crate::ID,
        executable: false,
        rent_epoch: 0,
    };

    let label = "Treasury";

    let instruction: Instruction = SetLabelInstruction {
        creator,
        config,
        system_program,
        label: label.as_bytes().to_vec(),
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (creator, creator_account),
            (config, config_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "set_label failed: {:?}",
        result.program_result
    );

    // Verify label was stored
    let config_data = &result.resulting_accounts[1].1.data;
    let label_end = u16::from_le_bytes([config_data[35], config_data[36]]) as usize;
    assert_eq!(label_end, label.len(), "label_end mismatch");

    let zc_header_size = 32 + 1 + 1 + 2 + 2; // 38
    let label_start = 1 + zc_header_size;
    let stored_label = core::str::from_utf8(&config_data[label_start..label_start + label_end])
        .expect("invalid UTF-8");
    assert_eq!(stored_label, label, "label content mismatch");

    std::println!("\n========================================");
    std::println!("  SET_LABEL CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_execute_transfer() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let creator = Address::new_unique();
    let creator_account = Account::default();

    let signer1 = Address::new_unique();
    let signer1_account = Account::default();
    let signer2 = Address::new_unique();
    let signer2_account = Account::default();
    let signer3 = Address::new_unique();

    let (config, config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_data = build_config_data(creator, 2, config_bump, "", &[signer1, signer2, signer3]);
    let config_account = Account {
        lamports: 1_000_000,
        data: config_data,
        owner: crate::ID,
        executable: false,
        rent_epoch: 0,
    };

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", config.as_ref()], &crate::ID);
    let vault_account = Account::new(5_000_000_000, 0, &system_program);

    let recipient = Address::new_unique();
    let recipient_account = Account::new(0, 0, &system_program);

    let transfer_amount: u64 = 1_000_000_000;

    // Build instruction with 2 signers as remaining accounts (meets threshold of 2)
    let mut instruction: Instruction = ExecuteTransferInstruction {
        config,
        creator,
        vault,
        recipient,
        system_program,
        amount: transfer_amount,
    }
    .into();

    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer1, true));
    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer2, true));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (config, config_account),
            (creator, creator_account),
            (vault, vault_account),
            (recipient, recipient_account),
            (system_program, system_program_account.clone()),
            (signer1, signer1_account),
            (signer2, signer2_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "execute_transfer failed: {:?}",
        result.program_result
    );

    let vault_after = result.resulting_accounts[2].1.lamports;
    let recipient_after = result.resulting_accounts[3].1.lamports;

    assert_eq!(
        vault_after,
        5_000_000_000 - transfer_amount,
        "vault lamports after transfer"
    );
    assert_eq!(
        recipient_after, transfer_amount,
        "recipient lamports after transfer"
    );

    std::println!("\n========================================");
    std::println!("  EXECUTE_TRANSFER CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_execute_transfer_insufficient_signers() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let creator = Address::new_unique();
    let creator_account = Account::default();

    let signer1 = Address::new_unique();
    let signer1_account = Account::default();
    let signer2 = Address::new_unique();
    let signer3 = Address::new_unique();

    let (config, config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_data = build_config_data(creator, 2, config_bump, "", &[signer1, signer2, signer3]);
    let config_account = Account {
        lamports: 1_000_000,
        data: config_data,
        owner: crate::ID,
        executable: false,
        rent_epoch: 0,
    };

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", config.as_ref()], &crate::ID);
    let vault_account = Account::new(5_000_000_000, 0, &system_program);

    let recipient = Address::new_unique();
    let recipient_account = Account::new(0, 0, &system_program);

    // Only 1 signer — threshold is 2, should fail
    let mut instruction: Instruction = ExecuteTransferInstruction {
        config,
        creator,
        vault,
        recipient,
        system_program,
        amount: 1_000_000_000,
    }
    .into();

    instruction
        .accounts
        .push(AccountMeta::new_readonly(signer1, true));

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (config, config_account),
            (creator, creator_account),
            (vault, vault_account),
            (recipient, recipient_account),
            (system_program, system_program_account),
            (signer1, signer1_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "should fail with insufficient signers"
    );

    std::println!("\n========================================");
    std::println!("  INSUFFICIENT_SIGNERS: correctly rejected");
    std::println!("========================================\n");
}

#[test]
fn test_invalid_utf8_label_rejected() {
    let mollusk = setup();

    let creator = Address::new_unique();
    let signer1 = Address::new_unique();

    let (config, config_bump) =
        Address::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let config_data = build_config_data_bytes(creator, 1, config_bump, &[0xFF, 0xFE], &[signer1]);
    let config_account = Account {
        lamports: 1_000_000,
        data: config_data,
        owner: crate::ID,
        executable: false,
        rent_epoch: 0,
    };

    let depositor = Address::new_unique();
    let depositor_account = Account::new(10_000_000_000, 0, &Address::default());

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", config.as_ref()], &crate::ID);
    let vault_account = Account::new(0, 0, &Address::default());

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let instruction: Instruction = DepositInstruction {
        depositor,
        config,
        vault,
        system_program,
        amount: 1_000,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (depositor, depositor_account),
            (config, config_account),
            (vault, vault_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "invalid UTF-8 label in config account should be rejected"
    );
}
