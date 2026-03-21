extern crate std;
use {
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    std::{println, vec},
};

fn program_id() -> Address {
    crate::ID
}

fn setup() -> Mollusk {
    Mollusk::new(&program_id(), "../../target/deploy/pinocchio_vault")
}

fn deposit_instruction(
    user: Address,
    vault: Address,
    system_program: Address,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(user, true),
        AccountMeta::new(vault, false),
        AccountMeta::new_readonly(system_program, false),
    ];
    let mut data = vec![0u8];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: program_id(),
        accounts,
        data,
    }
}

fn withdraw_instruction(user: Address, vault: Address, amount: u64) -> Instruction {
    let accounts = vec![AccountMeta::new(user, true), AccountMeta::new(vault, false)];
    let mut data = vec![1u8];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: program_id(),
        accounts,
        data,
    }
}

#[test]
fn test_deposit() {
    let mollusk = setup();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _bump) = Address::find_program_address(&[b"vault", user.as_ref()], &program_id());
    let vault_account = Account::new(0, 0, &system_program);

    let deposit_amount: u64 = 1_000_000_000;

    let instruction = deposit_instruction(user, vault, system_program, deposit_amount);

    let result = mollusk.process_instruction(
        &instruction,
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

    let user_after = result.resulting_accounts[0].1.lamports;
    let vault_after = result.resulting_accounts[1].1.lamports;

    assert_eq!(user_after, 10_000_000_000 - deposit_amount);
    assert_eq!(vault_after, deposit_amount);

    println!("\n========================================");
    println!("  DEPOSIT CU: {}", result.compute_units_consumed);
    println!("========================================\n");
}

#[test]
fn test_withdraw() {
    let mollusk = setup();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _bump) = Address::find_program_address(&[b"vault", user.as_ref()], &program_id());
    let vault_account = Account::new(0, 0, &program_id());

    let deposit_amount: u64 = 1_000_000_000;

    // First deposit
    let deposit_ix = deposit_instruction(user, vault, system_program, deposit_amount);

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

    let withdraw_ix = withdraw_instruction(user, vault, withdraw_amount);

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

    assert_eq!(user_final, user_after_deposit.lamports + withdraw_amount);
    assert_eq!(vault_final, deposit_amount - withdraw_amount);

    println!("\n========================================");
    println!("  WITHDRAW CU: {}", result.compute_units_consumed);
    println!("========================================\n");
}
