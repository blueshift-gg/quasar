extern crate std;
use {
    alloc::vec,
    quasar_lang::client::{DynBytes, DynVec},
    quasar_multisig_client::*,
    quasar_svm::{Account, Instruction, Pubkey, QuasarSvm},
    solana_instruction::AccountMeta,
    std::println,
};

fn setup() -> QuasarSvm {
    let elf = std::fs::read("../../target/deploy/quasar_multisig.so").unwrap();
    QuasarSvm::new().with_program(&crate::ID, &elf)
}

fn signer(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 10_000_000_000)
}

fn empty(address: Pubkey) -> Account {
    Account {
        address,
        lamports: 0,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }
}

fn config_account(
    address: Pubkey,
    creator: Pubkey,
    threshold: u8,
    bump: u8,
    label: &[u8],
    signers: &[Pubkey],
) -> Account {
    let config = MultisigConfig {
        creator,
        threshold,
        bump,
        label: DynBytes::<u8>::new(label.to_vec()),
        signers: DynVec::<Pubkey, u16>::new(signers.to_vec()),
    };
    Account {
        address,
        lamports: 1_000_000,
        data: wincode::serialize(&config).unwrap(),
        owner: crate::ID,
        executable: false,
    }
}

#[test]
fn test_create() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let creator = Pubkey::new_unique();
    let signer1 = Pubkey::new_unique();
    let signer2 = Pubkey::new_unique();
    let signer3 = Pubkey::new_unique();
    let (config, _) = Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);

    let threshold: u8 = 2;

    // Rent sysvar address
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

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

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(creator),
            empty(config),
            empty(signer1),
            empty(signer2),
            empty(signer3),
        ],
    );

    assert!(result.is_ok(), "create failed: {:?}", result.raw_result);

    // Verify config account data
    let config_data = &result.account(&config).unwrap().data;
    assert_eq!(config_data[0], 1, "discriminator should be 1");
    assert_eq!(config_data[33], threshold, "threshold mismatch");

    // Signers count prefix at offset 36 (disc(1) + ZC(34) + label_prefix(1) +
    // label(0))
    let signers_count = u16::from_le_bytes([config_data[36], config_data[37]]);
    assert_eq!(signers_count, 3, "signers count should be 3");

    println!("  CREATE CU: {}", result.compute_units_consumed);
}

#[test]
fn test_deposit() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let creator = Pubkey::new_unique();
    let signer1 = Pubkey::new_unique();
    let signer2 = Pubkey::new_unique();
    let depositor = Pubkey::new_unique();

    let (config, config_bump) =
        Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let (vault, _) = Pubkey::find_program_address(&[b"vault", config.as_ref()], &crate::ID);

    let deposit_amount: u64 = 1_000_000_000;

    let instruction: Instruction = DepositInstruction {
        depositor,
        config,
        vault,
        system_program,
        amount: deposit_amount,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(depositor),
            config_account(config, creator, 2, config_bump, b"", &[signer1, signer2]),
            empty(vault),
        ],
    );

    assert!(result.is_ok(), "deposit failed: {:?}", result.raw_result);

    let vault_after = result.account(&vault).unwrap().lamports;
    assert_eq!(vault_after, deposit_amount, "vault lamports after deposit");

    println!("  DEPOSIT CU: {}", result.compute_units_consumed);
}

#[test]
fn test_set_label() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let creator = Pubkey::new_unique();
    let signer1 = Pubkey::new_unique();
    let (config, config_bump) =
        Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);

    let label = "Treasury";

    let instruction: Instruction = SetLabelInstruction {
        creator,
        config,
        system_program,
        label: DynBytes::<u8>::new(label.as_bytes().to_vec()),
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(creator),
            config_account(config, creator, 1, config_bump, b"", &[signer1]),
        ],
    );

    assert!(result.is_ok(), "set_label failed: {:?}", result.raw_result);

    // Verify label was stored
    // Compact layout: disc(1) + header(37) + tail(label_data + signers_data)
    // Header: creator(32) + threshold(1) + bump(1) + label_len(1) + signers_len(2)
    // = 37
    let config_data = &result.account(&config).unwrap().data;
    let label_len = config_data[35] as usize; // label_len at offset 1+32+1+1 = 35
    assert_eq!(label_len, label.len(), "label length mismatch");

    let tail_start = 1 + 37; // disc + header
    let stored_label =
        core::str::from_utf8(&config_data[tail_start..tail_start + label_len]).unwrap();
    assert_eq!(stored_label, label, "label content mismatch");

    println!("  SET_LABEL CU: {}", result.compute_units_consumed);
}

#[test]
fn test_execute_transfer() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let creator = Pubkey::new_unique();
    let signer1 = Pubkey::new_unique();
    let signer2 = Pubkey::new_unique();
    let signer3 = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();

    let (config, config_bump) =
        Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let (vault, _) = Pubkey::find_program_address(&[b"vault", config.as_ref()], &crate::ID);

    let transfer_amount: u64 = 1_000_000_000;

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

    let vault_initial = 5_000_000_000u64;

    let result = svm.process_instruction(
        &instruction,
        &[
            config_account(
                config,
                creator,
                2,
                config_bump,
                b"",
                &[signer1, signer2, signer3],
            ),
            empty(creator),
            Account {
                address: vault,
                lamports: vault_initial,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            empty(recipient),
            empty(signer1),
            empty(signer2),
        ],
    );

    assert!(
        result.is_ok(),
        "execute_transfer failed: {:?}",
        result.raw_result
    );

    let vault_after = result.account(&vault).unwrap().lamports;
    let recipient_after = result.account(&recipient).unwrap().lamports;

    assert_eq!(
        vault_after,
        vault_initial - transfer_amount,
        "vault lamports after transfer"
    );
    assert_eq!(
        recipient_after, transfer_amount,
        "recipient lamports after transfer"
    );

    println!("  EXECUTE_TRANSFER CU: {}", result.compute_units_consumed);
}

#[test]
fn test_execute_transfer_insufficient_signers() {
    let mut svm = setup();

    let system_program = quasar_svm::system_program::ID;
    let creator = Pubkey::new_unique();
    let signer1 = Pubkey::new_unique();
    let signer2 = Pubkey::new_unique();
    let signer3 = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();

    let (config, config_bump) =
        Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &crate::ID);
    let (vault, _) = Pubkey::find_program_address(&[b"vault", config.as_ref()], &crate::ID);

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

    let result = svm.process_instruction(
        &instruction,
        &[
            config_account(
                config,
                creator,
                2,
                config_bump,
                b"",
                &[signer1, signer2, signer3],
            ),
            empty(creator),
            Account {
                address: vault,
                lamports: 5_000_000_000,
                data: vec![],
                owner: quasar_svm::system_program::ID,
                executable: false,
            },
            empty(recipient),
            empty(signer1),
        ],
    );

    assert!(result.is_err(), "should fail with insufficient signers");
    println!("  INSUFFICIENT_SIGNERS: correctly rejected");
}

// NOTE: UTF-8 re-validation was removed in Phase 7 (perf/cu-optimizations).
// The owner check already proves the account was written by this program,
// and all PodString write paths accept &str (valid UTF-8 by construction).
// Corrupted data in an owned account means Solana's security model is
// broken — not something the program needs to defend against.
