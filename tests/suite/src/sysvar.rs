use {
    crate::helpers::mollusk_for_program,
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    quasar_test_sysvar::cpi::*,
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
};

const CLOCK_SNAPSHOT_SIZE: usize = 17;
const RENT_SNAPSHOT_SIZE: usize = 9;
const CLOCK_FULL_SNAPSHOT_SIZE: usize = 41;
const RENT_CALC_SNAPSHOT_SIZE: usize = 9;

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_sysvar::ID, "quasar_test_sysvar")
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

#[test]
fn test_clock_custom_slot() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.slot = 12345;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_custom_slot failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), CLOCK_FULL_SNAPSHOT_SIZE, "snapshot size");
    assert_eq!(data[0], 3, "discriminator");
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    assert_eq!(slot, 12345, "slot should be 12345");
    println!(
        "  clock_custom_slot: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_clock_unix_timestamp() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.unix_timestamp = 1_700_000_000;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_unix_timestamp failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let unix_timestamp = i64::from_le_bytes(data[33..41].try_into().unwrap());
    assert_eq!(unix_timestamp, 1_700_000_000, "unix_timestamp mismatch");
    println!(
        "  clock_unix_timestamp: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_clock_epoch() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.epoch = 42;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_epoch failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let epoch = u64::from_le_bytes(data[17..25].try_into().unwrap());
    assert_eq!(epoch, 42, "epoch mismatch");
    println!("  clock_epoch: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_clock_epoch_start_timestamp() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.epoch_start_timestamp = 1_600_000_000;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_epoch_start_timestamp failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let epoch_start_timestamp = i64::from_le_bytes(data[9..17].try_into().unwrap());
    assert_eq!(
        epoch_start_timestamp, 1_600_000_000,
        "epoch_start_timestamp mismatch"
    );
    println!(
        "  clock_epoch_start_timestamp: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_clock_leader_schedule_epoch() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.leader_schedule_epoch = 99;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_leader_schedule_epoch failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let leader_schedule_epoch = u64::from_le_bytes(data[25..33].try_into().unwrap());
    assert_eq!(leader_schedule_epoch, 99, "leader_schedule_epoch mismatch");
    println!(
        "  clock_leader_schedule_epoch: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_clock_all_fields() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.slot = 500;
    mollusk.sysvars.clock.epoch_start_timestamp = 1_600_000_000;
    mollusk.sysvars.clock.epoch = 10;
    mollusk.sysvars.clock.leader_schedule_epoch = 11;
    mollusk.sysvars.clock.unix_timestamp = 1_700_000_000;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_all_fields failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), CLOCK_FULL_SNAPSHOT_SIZE, "snapshot size");
    assert_eq!(data[0], 3, "discriminator");
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let epoch_start_timestamp = i64::from_le_bytes(data[9..17].try_into().unwrap());
    let epoch = u64::from_le_bytes(data[17..25].try_into().unwrap());
    let leader_schedule_epoch = u64::from_le_bytes(data[25..33].try_into().unwrap());
    let unix_timestamp = i64::from_le_bytes(data[33..41].try_into().unwrap());
    assert_eq!(slot, 500, "slot");
    assert_eq!(
        epoch_start_timestamp, 1_600_000_000,
        "epoch_start_timestamp"
    );
    assert_eq!(epoch, 10, "epoch");
    assert_eq!(leader_schedule_epoch, 11, "leader_schedule_epoch");
    assert_eq!(unix_timestamp, 1_700_000_000, "unix_timestamp");
    println!(
        "  clock_all_fields: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_clock_large_values() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.slot = u64::MAX;
    mollusk.sysvars.clock.epoch = u64::MAX;
    mollusk.sysvars.clock.leader_schedule_epoch = u64::MAX;
    mollusk.sysvars.clock.unix_timestamp = i64::MAX;
    mollusk.sysvars.clock.epoch_start_timestamp = i64::MAX;
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
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
        "clock_large_values failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let slot = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let epoch = u64::from_le_bytes(data[17..25].try_into().unwrap());
    let leader_schedule_epoch = u64::from_le_bytes(data[25..33].try_into().unwrap());
    let unix_timestamp = i64::from_le_bytes(data[33..41].try_into().unwrap());
    let epoch_start_timestamp = i64::from_le_bytes(data[9..17].try_into().unwrap());
    assert_eq!(slot, u64::MAX, "slot max");
    assert_eq!(epoch, u64::MAX, "epoch max");
    assert_eq!(leader_schedule_epoch, u64::MAX, "leader_schedule_epoch max");
    assert_eq!(unix_timestamp, i64::MAX, "unix_timestamp max");
    assert_eq!(epoch_start_timestamp, i64::MAX, "epoch_start_timestamp max");
    println!(
        "  clock_large_values: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_rent_minimum_balance_small() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"rent_calc"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadRentCalcInstruction {
        payer,
        snapshot,
        system_program,
        data_len: 100,
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
        "rent_minimum_balance_small failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    assert_eq!(data.len(), RENT_CALC_SNAPSHOT_SIZE, "snapshot size");
    assert_eq!(data[0], 4, "discriminator");
    let min_balance = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let expected = mollusk.sysvars.rent.minimum_balance(100);
    assert_eq!(min_balance, expected, "min_balance for 100 bytes");
    println!(
        "  rent_minimum_balance_small: OK (CU: {}, min_balance={})",
        result.compute_units_consumed, min_balance
    );
}

#[test]
fn test_rent_minimum_balance_large() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"rent_calc"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadRentCalcInstruction {
        payer,
        snapshot,
        system_program,
        data_len: 10_000,
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
        "rent_minimum_balance_large failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let min_balance = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let expected = mollusk.sysvars.rent.minimum_balance(10_000);
    assert_eq!(min_balance, expected, "min_balance for 10000 bytes");
    println!(
        "  rent_minimum_balance_large: OK (CU: {}, min_balance={})",
        result.compute_units_consumed, min_balance
    );
}

#[test]
fn test_rent_minimum_balance_zero() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"rent_calc"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadRentCalcInstruction {
        payer,
        snapshot,
        system_program,
        data_len: 0,
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
        "rent_minimum_balance_zero failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let min_balance = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let expected = mollusk.sysvars.rent.minimum_balance(0);
    assert_eq!(min_balance, expected, "min_balance for 0 bytes");
    assert!(
        min_balance > 0,
        "min_balance for 0 bytes should be > 0 due to storage overhead"
    );
    println!(
        "  rent_minimum_balance_zero: OK (CU: {}, min_balance={})",
        result.compute_units_consumed, min_balance
    );
}

#[test]
fn test_rent_lamports_per_byte() {
    let mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"rent_calc"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadRentCalcInstruction {
        payer,
        snapshot,
        system_program,
        data_len: 1,
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
        "rent_lamports_per_byte failed: {:?}",
        result.program_result
    );
    let data = &result.resulting_accounts[1].1.data;
    let min_balance_1 = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let expected_1 = mollusk.sysvars.rent.minimum_balance(1);
    let expected_0 = mollusk.sysvars.rent.minimum_balance(0);
    assert_eq!(min_balance_1, expected_1, "min_balance for 1 byte");
    let per_byte = expected_1 - expected_0;
    assert!(per_byte > 0, "lamports per byte should be > 0");
    println!(
        "  rent_lamports_per_byte: OK (CU: {}, per_byte={})",
        result.compute_units_consumed, per_byte
    );
}

#[test]
fn test_clock_syscall_vs_account_consistent() {
    let mut mollusk = setup();
    let (system_program, system_program_account) = keyed_account_for_system_program();
    mollusk.sysvars.clock.slot = 777;
    mollusk.sysvars.clock.epoch_start_timestamp = 1_650_000_000;
    mollusk.sysvars.clock.epoch = 5;
    mollusk.sysvars.clock.leader_schedule_epoch = 6;
    mollusk.sysvars.clock.unix_timestamp = 1_650_100_000;

    let payer = Address::new_unique();
    let payer_account = Account::new(1_000_000_000, 0, &system_program);
    let (snapshot, _) = Address::find_program_address(&[b"clock_full"], &quasar_test_sysvar::ID);
    let snapshot_account = Account::default();
    let instruction: Instruction = ReadClockFullInstruction {
        payer,
        snapshot,
        system_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account.clone()),
            (snapshot, snapshot_account),
            (system_program, system_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "syscall read failed: {:?}",
        result.program_result
    );
    let syscall_data = result.resulting_accounts[1].1.data.clone();

    let mut snapshot_data = vec![0u8; CLOCK_FULL_SNAPSHOT_SIZE];
    snapshot_data[0] = 3;
    let snapshot_account = Account {
        lamports: 1_000_000,
        data: snapshot_data,
        owner: quasar_test_sysvar::ID,
        executable: false,
        rent_epoch: 0,
    };
    let (clock, clock_account) = mollusk.sysvars.keyed_account_for_clock_sysvar();
    let instruction: Instruction = ReadClockFullFromAccountInstruction {
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
        "account read failed: {:?}",
        result.program_result
    );
    let account_data = &result.resulting_accounts[1].1.data;

    assert_eq!(
        syscall_data, *account_data,
        "syscall and account clock data should match"
    );
    println!(
        "  clock_syscall_vs_account_consistent: OK (CU: {})",
        result.compute_units_consumed
    );
}
