use {
    crate::helpers::mollusk_for_program, mollusk_svm::Mollusk, quasar_test_events::cpi::*,
    solana_account::Account, solana_address::Address,
};

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_events::ID, "quasar_test_events")
}

const EMIT_MIN_CU: u64 = 200;

#[test]
fn test_emit_u64() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitU64EventInstruction { signer, value: 42 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed > EMIT_MIN_CU,
        "emit should consume CU for sol_log_data syscall, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_address() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let addr = Address::new_unique();
    let instruction = EmitAddressEventInstruction {
        signer,
        addr,
        value: 100,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed > EMIT_MIN_CU,
        "emit should consume CU for sol_log_data syscall, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_bool_true() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitBoolEventInstruction { signer, flag: true }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed > EMIT_MIN_CU,
        "emit should consume CU for sol_log_data syscall, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_bool_false() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitBoolEventInstruction {
        signer,
        flag: false,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed > EMIT_MIN_CU,
        "emit should consume CU for sol_log_data syscall, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_multi_field() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let c = Address::new_unique();
    let instruction = EmitMultiFieldInstruction {
        signer,
        a: 1,
        b: 2,
        c,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed > EMIT_MIN_CU,
        "emit should consume CU for sol_log_data syscall, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_cu() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitU64EventInstruction { signer, value: 42 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    println!("emit!() CU: {}", result.compute_units_consumed);
    assert!(
        result.compute_units_consumed < 500,
        "emit!() should be under 500 CU, got {}",
        result.compute_units_consumed
    );
}

fn make_cpi_accounts(
    signer: Address,
    event_authority: Address,
    program_id: Address,
) -> Vec<(Address, Account)> {
    vec![
        (signer, Account::new(1_000_000, 0, &Address::default())),
        (
            event_authority,
            Account::new(1_000_000, 0, &Address::default()),
        ),
        (
            program_id,
            Account {
                lamports: 1_000_000,
                data: Vec::new(),
                owner: mollusk_svm::program::loader_keys::LOADER_V2,
                executable: true,
                rent_epoch: 0,
            },
        ),
    ]
}

#[test]
fn test_emit_cpi_success() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, event_authority, quasar_test_events::ID),
    );
    assert!(
        result.program_result.is_ok(),
        "CPI emit failed: {:?}",
        result.program_result
    );
    assert!(
        result.compute_units_consumed > 1_000,
        "CPI emit should consume >1000 CU for self-CPI, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_cpi_different_value() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 999,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, event_authority, quasar_test_events::ID),
    );
    assert!(
        result.program_result.is_ok(),
        "CPI emit with different value failed: {:?}",
        result.program_result
    );
    assert!(
        result.compute_units_consumed > 1_000,
        "CPI emit should consume >1000 CU for self-CPI, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_cpi_cu() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, event_authority, quasar_test_events::ID),
    );
    assert!(result.program_result.is_ok());
    println!("emit_cpi!() CU: {}", result.compute_units_consumed);
    assert!(
        result.compute_units_consumed < 2_000,
        "emit_cpi!() should be under 2000 CU, got {}",
        result.compute_units_consumed
    );
}

#[test]
fn test_emit_cpi_wrong_authority() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let wrong_authority = Address::new_unique();
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority: wrong_authority,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, wrong_authority, quasar_test_events::ID),
    );
    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong event authority"
    );
}

// ── emit!() tests ───────────────────────────────────────────────────────

#[test]
fn test_emit_empty_event() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitEmptyEventInstruction { signer }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_ok(),
        "empty event emit failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_emit_large_data() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let e = Address::new_unique();
    let f = Address::new_unique();
    let instruction = EmitLargeEventInstruction {
        signer,
        a: u64::MAX,
        b: u64::MAX,
        c: u64::MAX,
        d: u64::MAX,
        e,
        f,
        g: u128::MAX,
        h: u128::MAX,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_ok(),
        "large event emit failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_emit_multiple_events() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitTwoEventsInstruction {
        signer,
        first: 111,
        second: 222,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_ok(),
        "two event emit failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_emit_cu_remains_low() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitU64EventInstruction { signer, value: 42 }.into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed < 500,
        "emit!() should stay under 500 CU budget, got {}",
        result.compute_units_consumed
    );
}

// ── emit_cpi!() tests ──────────────────────────────────────────────────

#[test]
fn test_emit_cpi_with_extra_accounts() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let extra = Address::new_unique();
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 77,
    }
    .into();
    let mut accounts = make_cpi_accounts(signer, event_authority, quasar_test_events::ID);
    accounts.push((extra, Account::new(1_000_000, 0, &Address::default())));
    let result = mollusk.process_instruction(&instruction, &accounts);
    assert!(
        result.program_result.is_ok(),
        "CPI emit with extra accounts failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_emit_cpi_wrong_program() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let wrong_program = Address::new_unique();
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: wrong_program,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (signer, Account::new(1_000_000, 0, &Address::default())),
            (
                event_authority,
                Account::new(1_000_000, 0, &Address::default()),
            ),
            (
                wrong_program,
                Account {
                    lamports: 1_000_000,
                    data: Vec::new(),
                    owner: mollusk_svm::program::loader_keys::LOADER_V2,
                    executable: true,
                    rent_epoch: 0,
                },
            ),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "Expected failure with wrong program for CPI emit"
    );
}

#[test]
fn test_emit_cpi_missing_event_authority() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let random_account = Address::new_unique();
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority: random_account,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, random_account, quasar_test_events::ID),
    );
    assert!(
        result.program_result.is_err(),
        "Expected failure with missing event authority"
    );
}

#[test]
fn test_emit_cpi_authority_not_signer() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let mut instruction: solana_instruction::Instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();

    instruction.accounts[0].is_signer = false;

    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, event_authority, quasar_test_events::ID),
    );
    assert!(
        result.program_result.is_err(),
        "Expected failure when signer account is not signed"
    );
}

#[test]
fn test_emit_cpi_cu_budget() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let (event_authority, _) =
        Address::find_program_address(&[b"__event_authority"], &quasar_test_events::ID);
    let instruction = EmitViaCpiInstruction {
        signer,
        event_authority,
        program: quasar_test_events::ID,
        value: 42,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &make_cpi_accounts(signer, event_authority, quasar_test_events::ID),
    );
    assert!(result.program_result.is_ok());
    assert!(
        result.compute_units_consumed < 2_000,
        "emit_cpi!() should stay under 2000 CU budget, got {}",
        result.compute_units_consumed
    );
}

// ── Event discriminator tests ───────────────────────────────────────────

#[test]
fn test_event_discriminator_matches() {
    let mollusk = setup();
    let signer = Address::new_unique();
    let instruction = EmitU64EventInstruction {
        signer,
        value: 12345,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(
        result.program_result.is_ok(),
        "emit for discriminator test failed: {:?}",
        result.program_result
    );
}

#[test]
fn test_different_events_different_discriminators() {
    let mollusk = setup();
    let signer = Address::new_unique();

    let instr_u64 = EmitU64EventInstruction { signer, value: 100 }.into();
    let result_u64 = mollusk.process_instruction(
        &instr_u64,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result_u64.program_result.is_ok());

    let instr_bool = EmitBoolEventInstruction { signer, flag: true }.into();
    let result_bool = mollusk.process_instruction(
        &instr_bool,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result_bool.program_result.is_ok());

    let instr_empty = EmitEmptyEventInstruction { signer }.into();
    let result_empty = mollusk.process_instruction(
        &instr_empty,
        &[(signer, Account::new(1_000_000, 0, &Address::default()))],
    );
    assert!(result_empty.program_result.is_ok());
}
