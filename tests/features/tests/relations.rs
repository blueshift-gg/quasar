use features::{Amount, Counter, FeatureError, ID, PINNED, Vault};
use quasar_lang::{Address, ProgramError, error::Error};
use quasar_test::Test;

fn vault(depositor: Address, bump: u8, amount: u64) -> Vault {
    Vault { depositor, bump, amount }
}

fn vault_address(depositor: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"vault", depositor.as_ref()], &ID)
}

#[test]
fn a_vault_at_its_pda_and_owned_by_the_signer_is_accepted() {
    let mut t = Test::new(ID, "features");
    let depositor = t.wallet(1_000_000);
    let (address, bump) = vault_address(&depositor);
    let vault = t.account_at(address, &vault(depositor, bump, 0));

    t.run(features::owned::instruction((depositor, vault), ())).succeeds();
}

#[test]
fn has_one_rejects_a_vault_stored_for_someone_else() {
    let mut t = Test::new(ID, "features");
    let depositor = t.wallet(1_000_000);
    let (address, bump) = vault_address(&depositor);
    let vault = t.account_at(address, &vault(Address::new_unique(), bump, 0));

    t.run(features::owned::instruction((depositor, vault), ())).fails(FeatureError::NotDepositor);
}

#[test]
fn a_fields_checks_run_in_the_order_written() {
    let mut t = Test::new(ID, "features");
    let depositor = t.wallet(1_000_000);
    let (_, bump) = vault_address(&depositor);
    // At the wrong address and stored for someone else: `has_one` is written first.
    let vault = t.account(&vault(Address::new_unique(), bump, 0));

    t.run(features::owned::instruction((depositor, vault), ())).fails(FeatureError::NotDepositor);
}

#[test]
fn has_one_without_an_error_fails_with_the_default_one() {
    let mut t = Test::new(ID, "features");
    let depositor = t.wallet(1_000_000);
    let (address, bump) = vault_address(&depositor);
    let vault = t.account_at(address, &vault(Address::new_unique(), bump, 5));

    t.run(features::withdraw_owned::instruction((depositor, vault), Amount { amount: 1 }))
        .fails(Error::HasOne);
}

#[test]
fn an_address_constraint_accepts_only_that_address() {
    let mut t = Test::new(ID, "features");
    let pinned = t.set(PINNED, Address::default(), 0, vec![]);
    let other = t.wallet(0);

    t.run(features::pinned::instruction(pinned, ())).succeeds();
    t.run(features::pinned::instruction(other, ())).fails(Error::Address);
}

#[test]
fn a_constraint_fails_with_its_declared_error_or_the_default_one() {
    let mut t = Test::new(ID, "features");
    let [empty, funded, full] =
        [0, 500, 5_000].map(|amount| t.account(&vault(Address::default(), 0, amount)));

    t.run(features::funded::instruction(funded, ())).succeeds();
    t.run(features::funded::instruction(empty, ())).fails(FeatureError::Empty);
    t.run(features::funded::instruction(full, ())).fails(Error::Constraint);
}

#[test]
fn a_literal_seed_pda_is_checked_against_its_build_time_address() {
    let mut t = Test::new(ID, "features");
    let counter = Counter { count: 0, authority: Address::default() };
    let (address, _) = Address::find_program_address(&[b"config"], &ID);
    let config = t.account_at(address, &counter);
    let elsewhere = t.account(&counter);

    t.run(features::global::instruction(config, ())).succeeds();
    t.run(features::global::instruction(elsewhere, ())).fails(ProgramError::InvalidSeeds);
}

#[test]
fn the_first_failing_field_decides_the_error() {
    let mut t = Test::new(ID, "features");
    let first = t.wallet(1_000_000);
    let second = t.account_with(Address::new_unique(), 1_000_000, vec![0; 41]);
    let mut ix = features::ordered::instruction((first, second), ());
    ix.accounts[0].is_signer = false;

    t.run(ix).fails(ProgramError::MissingRequiredSignature);
}

#[test]
fn a_check_naming_a_later_field_runs_once_that_field_has_loaded() {
    let mut t = Test::new(ID, "features");
    let depositor = t.wallet(1_000_000);
    let (address, bump) = vault_address(&depositor);
    let at_pda = t.account_at(address, &vault(depositor, bump, 0));
    let elsewhere = t.account(&vault(depositor, bump, 0));

    t.run(features::owned_later::instruction((at_pda, depositor), ())).succeeds();
    t.run(features::owned_later::instruction((elsewhere, depositor), ()))
        .fails(ProgramError::InvalidSeeds);

    // At the wrong address and unsigned: `depositor` loads before the check runs.
    let mut ix = features::owned_later::instruction((elsewhere, depositor), ());
    ix.accounts[1].is_signer = false;
    t.run(ix).fails(ProgramError::MissingRequiredSignature);
}
