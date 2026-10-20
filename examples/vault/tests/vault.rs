use quasar_lang::{Address, ProgramError};
use quasar_test::Test;
use vault::{Amount, ID};

const SYSTEM: Address = Address::new_from_array([0; 32]);

fn vault_of(user: &Address) -> Address {
    Address::find_program_address(&[b"vault", user.as_ref()], &ID).0
}

fn setup() -> (Test, Address, Address) {
    let mut t = Test::new(ID, "vault");
    let user = t.wallet(10_000_000_000);
    let vault = t.set(vault_of(&user), SYSTEM, 0, vec![]);
    (t, user, vault)
}

#[test]
fn a_deposit_moves_lamports_into_the_users_vault() {
    let (mut t, user, vault) = setup();

    t.run(vault::deposit::instruction((user, vault, SYSTEM), Amount { amount: 1_000_000 }))
        .succeeds();

    assert_eq!(t.lamports(&vault), 1_000_000);
    assert_eq!(t.lamports(&user), 10_000_000_000 - 1_000_000);
}

#[test]
fn a_withdrawal_returns_lamports_to_the_user() {
    let (mut t, user, vault) = setup();
    t.run(vault::deposit::instruction((user, vault, SYSTEM), Amount { amount: 1_000_000 }))
        .succeeds();

    t.run(vault::withdraw::instruction((user, vault, SYSTEM), Amount { amount: 400_000 }))
        .succeeds();

    assert_eq!(t.lamports(&vault), 600_000);
}

#[test]
fn a_vault_of_another_user_is_rejected() {
    let (mut t, user, _) = setup();
    let stranger = t.wallet(1_000_000);
    let other_vault = t.set(vault_of(&stranger), SYSTEM, 5_000_000, vec![]);

    t.run(vault::withdraw::instruction((user, other_vault, SYSTEM), Amount { amount: 1 }))
        .fails(ProgramError::InvalidSeeds);
}

#[test]
fn a_vault_at_any_other_bump_is_rejected() {
    // Below the canonical bump, the seeds hash to addresses off the curve,
    // which anyone can fund, and on it, which no program signs for.
    let (mut t, user, vault) = setup();
    for bump in 0..=u8::MAX {
        let address = Address::derive_address_const(&[b"vault", user.as_ref()], Some(bump), &ID);
        if address != vault {
            t.set(address, SYSTEM, 0, vec![]);
            t.run(vault::deposit::instruction((user, address, SYSTEM), Amount { amount: 1 }))
                .fails(ProgramError::InvalidSeeds);
        }
    }
}

#[test]
fn a_withdrawal_needs_the_users_signature() {
    let (mut t, user, vault) = setup();
    t.run(vault::deposit::instruction((user, vault, SYSTEM), Amount { amount: 1_000_000 }))
        .succeeds();
    let mut ix = vault::withdraw::instruction((user, vault, SYSTEM), Amount { amount: 1 });
    ix.accounts[0].is_signer = false;

    t.run(ix).fails(ProgramError::MissingRequiredSignature);
}

#[test]
fn the_idl_matches_the_committed_one() {
    let idl = vault::idl().to_json();
    quasar_test::snapshot(concat!(env!("CARGO_MANIFEST_DIR"), "/idl.json"), &idl);
}
