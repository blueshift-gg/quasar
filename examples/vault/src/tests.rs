extern crate std;
use {quasar_test::prelude::*, quasar_vault_client::*};

const USER: Pubkey = Pubkey::new_from_array([1; 32]);
const MAX_ELF_BYTES: usize = 6_600;
const MAX_DEPOSIT_CU: u64 = 1_556;
const MAX_WITHDRAW_CU: u64 = 1_600;

#[test]
fn elf_size_stays_within_budget() {
    let bytes = std::fs::read("../../target/deploy/quasar_vault.so").unwrap();
    assert!(
        bytes.len() <= MAX_ELF_BYTES,
        "vault ELF grew to {} bytes; budget is {MAX_ELF_BYTES}",
        bytes.len()
    );
}

#[quasar_test]
fn deposit_creates_and_funds_the_vault() {
    ctx.add(Wallet::account().at(USER));
    let vault = find_vault_address(&USER, &crate::ID).0;
    let deposit = 1_000_000_000;

    ctx.execute(DepositInstruction {
        user: USER,
        amount: deposit,
    })
    .checks([
        Outcome::success(),
        Cu::spent(|cu| cu <= MAX_DEPOSIT_CU),
        Account::lamports(vault, deposit),
        Account::lamports(USER, DEFAULT_WALLET_LAMPORTS - deposit),
        Account::created(vault),
    ]);
}

#[quasar_test]
fn failed_init_does_not_leave_a_placeholder() {
    ctx.add(Wallet::account().at(USER));
    let wrong_vault = Pubkey::new_from_array([99; 32]);

    let outcome = ctx.execute(DepositInstructionRaw {
        user: USER,
        vault: wrong_vault,
        amount: 1,
    });

    outcome.check(Outcome::error(QuasarVaultError::InvalidPda));
    assert!(ctx.account(wrong_vault).is_none());
    assert!(outcome.account_changes().is_empty());
}

#[test]
fn compute_exhaustion_has_the_same_stable_error_as_typescript() {
    let mut ctx = Ctx::builder(crate::ID)
        .crate_name(env!("CARGO_PKG_NAME"))
        .compute_unit_limit(1)
        .build()
        .unwrap();
    ctx.add(Wallet::account().at(USER));

    ctx.execute(DepositInstruction {
        user: USER,
        amount: 1,
    })
    .check(Outcome::error(ProgramError::Runtime(
        "ProgramFailedToComplete".into(),
    )));
}

#[quasar_test]
fn withdraw_moves_lamports_out_of_program_state() {
    ctx.add(Wallet::account().at(USER));
    let vault = find_vault_address(&USER, &crate::ID).0;
    let vault_lamports = 1_000_000_000;
    let withdrawal = 500_000_000;
    // Deposit leaves the vault as a system-owned PDA holding lamports; the
    // withdraw CPI transfers out of it with the vault's seeds signing.
    ctx.add(Wallet::account().at(vault).fund(vault_lamports));

    ctx.simulate(WithdrawInstruction {
        user: USER,
        amount: withdrawal,
    })
    .check(Outcome::success())
    .checks([
        Cu::spent(|cu| cu <= MAX_WITHDRAW_CU),
        Account::lamports(USER, DEFAULT_WALLET_LAMPORTS + withdrawal),
        Account::lamports(vault, vault_lamports - withdrawal),
    ]);
    assert_eq!(ctx.lamports(USER), DEFAULT_WALLET_LAMPORTS);
    assert_eq!(ctx.lamports(vault), vault_lamports);

    ctx.execute(WithdrawInstruction {
        user: USER,
        amount: withdrawal,
    })
    .check(Outcome::success())
    .checks([
        Cu::spent(|cu| cu <= MAX_WITHDRAW_CU),
        Account::lamports(USER, DEFAULT_WALLET_LAMPORTS + withdrawal),
        Account::lamports(vault, vault_lamports - withdrawal),
    ]);
}
