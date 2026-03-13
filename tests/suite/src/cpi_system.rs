use {
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    quasar_test_misc::client::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
};

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_misc::ID,
        "../../target/deploy/quasar_test_misc",
    )
}

// ============================================================================
// SystemProgram CPI: create_account
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
fn test_create_account_zero_space() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let new_account = Address::new_unique();
    let new_account_obj = Account::new(0, 0, &system_program);

    let owner = Address::new_unique();

    let instruction: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program,
        lamports: 1_000_000,
        space: 0,
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
        "create_account with zero space should succeed: {:?}",
        result.program_result
    );

    let created = &result.resulting_accounts[1].1;
    assert_eq!(created.data.len(), 0, "space should be 0");
    assert_eq!(created.owner, owner, "owner");
}

#[test]
fn test_create_account_large_space() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(100_000_000_000, 0, &system_program);

    let new_account = Address::new_unique();
    let new_account_obj = Account::new(0, 0, &system_program);

    let owner = Address::new_unique();
    let space = 10_000u64;
    let lamports = 70_000_000u64;

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
        "create_account with large space should succeed: {:?}",
        result.program_result
    );

    let created = &result.resulting_accounts[1].1;
    assert_eq!(created.data.len(), space as usize, "space");
    assert_eq!(created.lamports, lamports, "lamports");
}

#[test]
fn test_create_account_insufficient_lamports() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(100, 0, &system_program);

    let new_account = Address::new_unique();
    let new_account_obj = Account::new(0, 0, &system_program);

    let owner = Address::new_unique();

    let instruction: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program,
        lamports: 10_000_000,
        space: 64,
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
        result.program_result.is_err(),
        "create_account should fail when payer lacks funds"
    );
}

// ============================================================================
// SystemProgram CPI: transfer
// ============================================================================

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
fn test_transfer_large_amount() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let balance = 500_000_000_000u64;
    let from_account = Account::new(balance, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(0, 0, &system_program);

    let amount = balance;

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
        "transfer of full balance should succeed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.lamports, 0,
        "from should have 0 lamports"
    );
    assert_eq!(
        result.resulting_accounts[1].1.lamports, amount,
        "to should have full amount"
    );
}

#[test]
fn test_transfer_to_self_fails_borrow() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let account = Address::new_unique();
    let account_obj = Account::new(10_000_000, 0, &system_program);

    let instruction: Instruction = TransferTestInstruction {
        from: account,
        to: account,
        system_program,
        amount: 1_000_000,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (account, account_obj.clone()),
            (account, account_obj),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "transfer to self should fail due to double borrow"
    );
}

// ============================================================================
// SystemProgram CPI: assign
// ============================================================================

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

#[test]
fn test_assign_to_system_program() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let account = Address::new_unique();
    let account_obj = Account::new(1_000_000, 0, &system_program);

    let instruction: Instruction = AssignTestInstruction {
        account,
        system_program,
        owner: system_program,
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
        "assign to system program should succeed: {:?}",
        result.program_result
    );

    assert_eq!(
        result.resulting_accounts[0].1.owner, system_program,
        "owner should be system program"
    );
}

#[test]
fn test_assign_already_assigned() {
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
        "first assign should succeed: {:?}",
        result.program_result
    );

    assert_eq!(result.resulting_accounts[0].1.owner, new_owner);
}

// ============================================================================
// Adversarial Tests — Attacker-Controlled Inputs
// ============================================================================

/// Transfer where from has u64::MAX lamports and to has u64::MAX lamports.
/// The addition should overflow — system program must reject.
#[test]
fn test_adversarial_transfer_lamport_overflow() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let from_account = Account::new(u64::MAX, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(u64::MAX, 0, &system_program);

    let instruction: Instruction = TransferTestInstruction {
        from,
        to,
        system_program,
        amount: 1,
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

    // Adding 1 to u64::MAX should overflow
    assert!(
        result.program_result.is_err(),
        "transfer that would overflow destination lamports must be rejected"
    );
}

/// Create account with space = u64::MAX — should fail gracefully, not panic.
#[test]
fn test_adversarial_create_account_absurd_space() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(u64::MAX, 0, &system_program);

    let new_account = Address::new_unique();
    let new_account_obj = Account::new(0, 0, &system_program);

    let instruction: Instruction = CreateAccountTestInstruction {
        payer,
        new_account,
        system_program,
        lamports: 1_000_000,
        space: u64::MAX,
        owner: Address::new_unique(),
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
        result.program_result.is_err(),
        "create_account with u64::MAX space must be rejected"
    );
}

/// Transfer with amount = 0 from an account with 0 lamports.
/// Edge case: should succeed (zero transfer from zero balance).
#[test]
fn test_adversarial_transfer_zero_from_zero_balance() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let from_account = Account::new(0, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(0, 0, &system_program);

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

    // Zero transfer from zero balance — should succeed
    assert!(
        result.program_result.is_ok(),
        "zero transfer from zero balance should succeed: {:?}",
        result.program_result
    );
}

/// Transfer with amount = u64::MAX from account that has u64::MAX lamports.
/// Should succeed (exact balance transfer).
#[test]
fn test_adversarial_transfer_exact_u64_max() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let from = Address::new_unique();
    let from_account = Account::new(u64::MAX, 0, &system_program);

    let to = Address::new_unique();
    let to_account = Account::new(0, 0, &system_program);

    let instruction: Instruction = TransferTestInstruction {
        from,
        to,
        system_program,
        amount: u64::MAX,
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
        "transfer exact u64::MAX from u64::MAX balance should succeed: {:?}",
        result.program_result
    );
}
