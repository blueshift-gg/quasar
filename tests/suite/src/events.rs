use mollusk_svm::Mollusk;
use quasar_test_events::client::*;
use solana_account::Account;
use solana_address::Address;

fn setup() -> Mollusk {
    Mollusk::new(
        &quasar_test_events::ID,
        "../../target/deploy/quasar_test_events",
    )
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
