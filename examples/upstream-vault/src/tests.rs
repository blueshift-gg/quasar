extern crate std;

use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

use crate::client::{DepositInstruction, WithdrawInstruction};

fn setup() -> Mollusk {
    Mollusk::new(&crate::ID, "../../target/deploy/upstream_vault")
}

#[test]
fn test_deposit() {
    let mollusk = setup();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", user.as_ref()], &crate::ID);
    let vault_account = Account::new(0, 0, &system_program);

    let deposit_amount: u64 = 1_000_000_000;

    let instruction: Instruction = DepositInstruction {
        user,
        vault,
        system_program,
        amount: deposit_amount,
    }
    .into();

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

    std::println!("\n========================================");
    std::println!("  DEPOSIT CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_withdraw() {
    let mollusk = setup();

    let (system_program, system_program_account) = keyed_account_for_system_program();

    let user = Address::new_unique();
    let user_account = Account::new(10_000_000_000, 0, &system_program);

    let (vault, _vault_bump) =
        Address::find_program_address(&[b"vault", user.as_ref()], &crate::ID);
    let vault_account = Account::new(0, 0, &crate::ID);

    let deposit_amount: u64 = 1_000_000_000;

    // First deposit
    let deposit_ix: Instruction = DepositInstruction {
        user,
        vault,
        system_program,
        amount: deposit_amount,
    }
    .into();

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

    let withdraw_ix: Instruction = WithdrawInstruction {
        user,
        vault,
        amount: withdraw_amount,
    }
    .into();

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

    std::println!("\n========================================");
    std::println!("  WITHDRAW CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}
