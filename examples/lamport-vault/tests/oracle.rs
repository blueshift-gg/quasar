//! The vault against Blueshift Quasar's vault example, built unmodified from a
//! pinned revision by `bench/rivals.sh`: the same instructions from the same
//! state must succeed or fail alike and leave the same accounts.

use lamport_vault::{Amount, ID};
use quasar_lang::{Address, client::Instruction};
use quasar_test::Test;

const SYSTEM: Address = Address::new_from_array([0; 32]);
const USER: Address = Address::new_from_array([7; 32]);
const STRANGER: Address = Address::new_from_array([8; 32]);

fn vault_of(user: &Address) -> Address {
    Address::find_program_address(&[b"vault", user.as_ref()], &ID).0
}

fn prepare(t: &mut Test) {
    t.set(USER, SYSTEM, 10_000_000_000, vec![]);
    t.set(STRANGER, SYSTEM, 1_000_000, vec![]);
    t.set(vault_of(&USER), ID, 0, vec![]);
    t.set(vault_of(&STRANGER), ID, 5_000_000, vec![]);
}

fn scenarios() -> Vec<(&'static str, Vec<Instruction>)> {
    let deposit = (USER, vault_of(&USER), SYSTEM);
    let withdraw = (USER, vault_of(&USER));
    let unsigned = {
        let mut ix = lamport_vault::withdraw::instruction(withdraw, Amount { amount: 1 });
        ix.accounts[0].is_signer = false;
        ix
    };
    vec![
        (
            "deposit",
            vec![lamport_vault::deposit::instruction(deposit, Amount { amount: 1_000_000 })],
        ),
        (
            "deposit then withdraw",
            vec![
                lamport_vault::deposit::instruction(deposit, Amount { amount: 1_000_000 }),
                lamport_vault::withdraw::instruction(withdraw, Amount { amount: 100_000 }),
            ],
        ),
        (
            "withdraw more than deposited",
            vec![lamport_vault::withdraw::instruction(withdraw, Amount { amount: 1 })],
        ),
        (
            "withdraw from another user's vault",
            vec![lamport_vault::withdraw::instruction(
                (USER, vault_of(&STRANGER)),
                Amount { amount: 1 },
            )],
        ),
        ("withdraw without the user's signature", vec![unsigned]),
    ]
}

#[test]
fn every_scenario_matches_blueshift_quasar() {
    for (name, instructions) in scenarios() {
        let mut ours = Test::new(ID, "lamport_vault");
        let mut theirs = Test::rival(ID, "quasar_vault");
        prepare(&mut ours);
        prepare(&mut theirs);
        for instruction in instructions {
            let (a, b) = (ours.run(instruction.clone()).is_ok(), theirs.run(instruction).is_ok());
            assert_eq!(a, b, "{name}: we succeeded: {a}, Blueshift Quasar succeeded: {b}");
        }
        assert_eq!(ours.state(), theirs.state(), "{name}: account state differs");
    }
}

#[test]
fn the_idl_matches_the_committed_one() {
    let idl = lamport_vault::idl().to_json();
    quasar_test::snapshot(concat!(env!("CARGO_MANIFEST_DIR"), "/idl.json"), &idl);
}
