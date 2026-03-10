use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use solana_account::Account;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

fn setup() -> Mollusk {
    let id_bytes: [u8; 32] = crate::ID.to_bytes();
    let program_id = Address::new_from_array(id_bytes);
    Mollusk::new(&program_id, "../../target/deploy/anchor_vault")
}

fn program_id() -> Address {
    Address::new_from_array(crate::ID.to_bytes())
}

/// Anchor discriminator = sha256("global:<name>")[..8]
fn deposit_ix_data(amount: u64) -> Vec<u8> {
    let mut data = vec![0xf2, 0x23, 0xc6, 0x89, 0x52, 0xe1, 0xf2, 0xb6];
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

fn withdraw_ix_data(amount: u64) -> Vec<u8> {
    let mut data = vec![0xb7, 0x12, 0x46, 0x9c, 0x94, 0x6d, 0xa1, 0x22];
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

#[test]
fn test_deposit() {
    let mollusk = setup();
    let pid = program_id();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _vault_bump) = Address::find_program_address(&[b"vault", user.as_ref()], &pid);
    let vault_account = Account::new(0, 0, &system_program);

    let deposit_amount: u64 = 1_000_000_000;

    let instruction = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(user, true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: deposit_ix_data(deposit_amount),
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (user, user_account.clone()),
            (vault, vault_account.clone()),
            (system_program, system_program_account.clone()),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "deposit failed: {:?}",
        result.program_result
    );

    let user_after = result.resulting_accounts[0].1.lamports;
    let vault_after = result.resulting_accounts[1].1.lamports;

    assert_eq!(
        user_after,
        10_000_000_000 - deposit_amount,
        "user lamports after deposit"
    );
    assert_eq!(vault_after, deposit_amount, "vault lamports after deposit");

    println!("\n========================================");
    println!("  ANCHOR DEPOSIT CU: {}", result.compute_units_consumed);
    println!("========================================\n");
}

#[test]
fn test_withdraw() {
    let mollusk = setup();
    let pid = program_id();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _vault_bump) = Address::find_program_address(&[b"vault", user.as_ref()], &pid);
    let vault_account = Account::new(0, 0, &pid);

    let deposit_amount: u64 = 1_000_000_000;

    // First deposit
    let deposit_ix = Instruction {
        program_id: pid,
        accounts: vec![
            AccountMeta::new(user, true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: deposit_ix_data(deposit_amount),
    };

    let result = mollusk.process_instruction(
        &deposit_ix,
        &[
            (user, user_account),
            (vault, vault_account),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "deposit failed: {:?}",
        result.program_result
    );

    let user_after_deposit = result.resulting_accounts[0].1.clone();
    let vault_after_deposit = result.resulting_accounts[1].1.clone();

    // Now withdraw
    let withdraw_amount: u64 = 500_000_000;

    let withdraw_ix = Instruction {
        program_id: pid,
        accounts: vec![AccountMeta::new(user, true), AccountMeta::new(vault, false)],
        data: withdraw_ix_data(withdraw_amount),
    };

    let result = mollusk.process_instruction(
        &withdraw_ix,
        &[
            (user, user_after_deposit.clone()),
            (vault, vault_after_deposit),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "withdraw failed: {:?}",
        result.program_result
    );

    let user_final = result.resulting_accounts[0].1.lamports;
    let vault_final = result.resulting_accounts[1].1.lamports;

    assert_eq!(
        user_final,
        user_after_deposit.lamports + withdraw_amount,
        "user lamports after withdraw"
    );
    assert_eq!(
        vault_final,
        deposit_amount - withdraw_amount,
        "vault lamports after withdraw"
    );

    println!("\n========================================");
    println!("  ANCHOR WITHDRAW CU: {}", result.compute_units_consumed);
    println!("========================================\n");
}
