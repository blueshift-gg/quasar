use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};
use quasar_test_sysvar::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

const CLOCK_SNAPSHOT_SIZE: usize = 17;
const RENT_SNAPSHOT_SIZE: usize = 9;

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_sysvar::ID,
        "../../target/deploy/quasar_test_sysvar",
    )
}

#[test]
fn test_read_clock_syscall() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.warp_to_slot(42);
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockInstruction {
        payer,
        snapshot,
        system_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (snapshot, snapshot_account),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "read_clock failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), CLOCK_SNAPSHOT_SIZE, "snapshot size");
    assert_eq!(data[0], 1, "discriminator");
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(slot, 42, "slot should be 42 after warp_to_slot(42)");
    println!(
        "  read_clock (syscall): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_read_clock_default_slot() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockInstruction {
        payer,
        snapshot,
        system_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (snapshot, snapshot_account),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "read_clock default failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(slot, 0, "default slot should be 0");
    println!(
        "  read_clock (default): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_read_rent_syscall() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"rent"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadRentInstruction {
        payer,
        snapshot,
        system_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (snapshot, snapshot_account),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "read_rent failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), RENT_SNAPSHOT_SIZE, "snapshot size");
    assert_eq!(data[0], 2, "discriminator");
    let min_balance = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert!(
        min_balance > 0,
        "min_balance for 100 bytes should be > 0, got {}",
        min_balance
    );
    println!(
        "  read_rent (syscall): OK (CU: {}, min_balance_100={})",
        result.compute_units_consumed, min_balance
    );
}

#[test]
fn test_read_clock_from_account() {
    let mut mollusk = setup();
    let (system_program, _) = keyed_account_for_system_program();
    mollusk.warp_to_slot(100);
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock"], &quasar_test_sysvar::ID);
    let mut snapshot_data = vec![0u8; CLOCK_SNAPSHOT_SIZE];
    snapshot_data[0] = 1;
    let snapshot_account = Account {
        lamports: 1_000_000,
        data: snapshot_data,
        owner: quasar_test_sysvar::ID,
        executable: false,
        rent_epoch: 0,
    };
    let (clock, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
    let instruction: Instruction = ReadClockFromAccountInstruction {
        _payer: payer,
        snapshot,
        clock,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (snapshot, snapshot_account),
            (clock, clock_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "read_clock_from_account failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(slot, 100, "slot should be 100 after warp_to_slot(100)");
    println!(
        "  read_clock (account): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_read_clock_account_after_warp() {
    let mut mollusk = setup();
    let (system_program, _) = keyed_account_for_system_program();
    mollusk.warp_to_slot(999);
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock"], &quasar_test_sysvar::ID);
    let mut snapshot_data = vec![0u8; CLOCK_SNAPSHOT_SIZE];
    snapshot_data[0] = 1;
    let snapshot_account = Account {
        lamports: 1_000_000,
        data: snapshot_data,
        owner: quasar_test_sysvar::ID,
        executable: false,
        rent_epoch: 0,
    };
    let (clock, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
    let instruction: Instruction = ReadClockFromAccountInstruction {
        _payer: payer,
        snapshot,
        clock,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (snapshot, snapshot_account),
            (clock, clock_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "read_clock_from_account after warp failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(slot, 999, "slot should be 999");
    println!(
        "  read_clock (account, warp=999): OK (CU: {})",
        result.compute_units_consumed
    );
}
