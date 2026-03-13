extern crate std;

use {
    crate::idl_client::{
        CreateInstruction, DepositInstruction, ExecuteTransferInstruction, SetLabelInstruction,
    },
    alloc::{vec, vec::Vec},
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
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
    // Layout: disc(1) + ZC fixed(34) + label_prefix(u32) + label_data +
    // signers_prefix(u32) + signers_data
    let total = 1 + 34 + 4 + label.len() + 4 + signers.len() * 32;
    let mut data = vec![0u8; total];

    // Discriminator
    data[0] = 1;

    // ZC fixed fields at offset 1
    data[1..33].copy_from_slice(creator.as_ref());
    data[33] = threshold;
    data[34] = bump;

    // Label prefix (u32 LE): byte length
    let label_len = label.len() as u32;
    data[35..39].copy_from_slice(&label_len.to_le_bytes());
    // Label data
    data[39..39 + label.len()].copy_from_slice(label);

    // Signers prefix (u32 LE): element count
    let signers_offset = 39 + label.len();
    let signers_count = signers.len() as u32;
    data[signers_offset..signers_offset + 4].copy_from_slice(&signers_count.to_le_bytes());
    // Signers data
    let signers_data_start = signers_offset + 4;
    for (i, signer) in signers.iter().enumerate() {
        data[signers_data_start + i * 32..signers_data_start + (i + 1) * 32]
            .copy_from_slice(signer.as_ref());
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
    let instruction: Instruction = CreateInstruction {
        creator,
        config,
        rent,
        system_program,
        threshold,
        remaining_accounts: vec![
            AccountMeta::new_readonly(signer1, true),
            AccountMeta::new_readonly(signer2, true),
            AccountMeta::new_readonly(signer3, true),
        ],
    }
    .into();

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

    // Verify signers count prefix (offset: disc(1) + ZC(34) + label_prefix(4) +
    // label(0) = 39)
    let signers_count = u32::from_le_bytes([
        config_data[39],
        config_data[40],
        config_data[41],
        config_data[42],
    ]);
    assert_eq!(signers_count, 3, "signers count should be 3");

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
    // Label prefix at offset 35 (disc(1) + ZC(34))
    let label_len = u32::from_le_bytes([
        config_data[35],
        config_data[36],
        config_data[37],
        config_data[38],
    ]) as usize;
    assert_eq!(label_len, label.len(), "label length mismatch");

    let label_start = 39; // disc(1) + ZC(34) + label_prefix(4)
    let stored_label = core::str::from_utf8(&config_data[label_start..label_start + label_len])
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
    let instruction: Instruction = ExecuteTransferInstruction {
        config,
        creator,
        vault,
        recipient,
        system_program,
        amount: transfer_amount,
        remaining_accounts: vec![
            AccountMeta::new_readonly(signer1, true),
            AccountMeta::new_readonly(signer2, true),
        ],
    }
    .into();

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
    let instruction: Instruction = ExecuteTransferInstruction {
        config,
        creator,
        vault,
        recipient,
        system_program,
        amount: 1_000_000_000,
        remaining_accounts: vec![AccountMeta::new_readonly(signer1, true)],
    }
    .into();

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
