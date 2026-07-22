extern crate std;
use {alloc::vec::Vec, quasar_test::prelude::*, quasar_vault_client::*};

const USER: Pubkey = Pubkey::new_from_array([1; 32]);
const MAX_ELF_BYTES: usize = 5_536;
const MAX_DEPOSIT_CU: u64 = 1_556;
const MAX_WITHDRAW_CU: u64 = 392;

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
fn deposit_creates_and_funds_the_vault(test: &mut Test) {
    test.add(Wallet::new().at(USER));
    let vault = find_vault_address(&USER, &crate::ID).0;
    let deposit = 1_000_000_000;

    let outcome = test.send(DepositInstruction {
        user: USER,
        amount: deposit,
    });

    outcome
        .succeeds()
        .cu_at_most(MAX_DEPOSIT_CU)
        .has_lamports(vault, deposit)
        .has_lamports(USER, DEFAULT_WALLET_LAMPORTS - deposit);
    assert!(
        outcome
            .account_changes()
            .iter()
            .any(|change| change.address() == vault && change.was_created()),
        "the outcome should identify the initialized vault"
    );
}

#[quasar_test]
fn failed_init_does_not_leave_a_placeholder(test: &mut Test) {
    test.add(Wallet::new().at(USER));
    let wrong_vault = Pubkey::new_from_array([99; 32]);

    let outcome = test.send(DepositInstructionRaw {
        user: USER,
        vault: wrong_vault,
        amount: 1,
    });

    outcome.fails_with(QuasarVaultError::InvalidPda);
    assert!(test.account(wrong_vault).is_none());
    assert!(outcome.account_changes().is_empty());
}

#[test]
fn compute_exhaustion_has_the_same_stable_error_as_typescript() {
    let mut test = Test::builder(crate::ID)
        .crate_name(env!("CARGO_PKG_NAME"))
        .compute_unit_limit(1)
        .build()
        .unwrap();
    test.add(Wallet::new().at(USER));

    test.send(DepositInstruction {
        user: USER,
        amount: 1,
    })
    .fails(ProgramError::Runtime("ProgramFailedToComplete".into()));
}

#[quasar_test]
fn withdraw_moves_lamports_out_of_program_state(test: &mut Test) {
    test.add(Wallet::new().at(USER));
    let vault = find_vault_address(&USER, &crate::ID).0;
    let vault_lamports = 1_000_000_000;
    let withdrawal = 500_000_000;
    test.add(Account::new(vault, crate::ID, vault_lamports, Vec::new()));

    test.simulate(WithdrawInstruction {
        user: USER,
        amount: withdrawal,
    })
    .succeeds()
    .cu_at_most(MAX_WITHDRAW_CU)
    .has_lamports(USER, DEFAULT_WALLET_LAMPORTS + withdrawal)
    .has_lamports(vault, vault_lamports - withdrawal);
    assert_eq!(test.lamports(USER), DEFAULT_WALLET_LAMPORTS);
    assert_eq!(test.lamports(vault), vault_lamports);

    test.send(WithdrawInstruction {
        user: USER,
        amount: withdrawal,
    })
    .succeeds()
    .cu_at_most(MAX_WITHDRAW_CU)
    .has_lamports(USER, DEFAULT_WALLET_LAMPORTS + withdrawal)
    .has_lamports(vault, vault_lamports - withdrawal);
}
