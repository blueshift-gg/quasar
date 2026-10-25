//! The system-owned vault against the same program using public Quasar master,
//! built by `bench/rivals.sh`: the same instructions from the same state must succeed
//! or fail alike and leave the same accounts.

use quasar_lang::{Address, client::Instruction};
use quasar_test::Test;
use vault::{Amount, ID};

const SYSTEM: Address = Address::new_from_array([0; 32]);
const USER: Address = Address::new_from_array([7; 32]);
const STRANGER: Address = Address::new_from_array([8; 32]);

fn vault_of(user: &Address) -> Address {
    Address::find_program_address(&[b"vault", user.as_ref()], &ID).0
}

fn prepare(t: &mut Test) {
    t.set(USER, SYSTEM, 10_000_000_000, vec![]);
    t.set(STRANGER, SYSTEM, 1_000_000, vec![]);
    t.set(vault_of(&USER), SYSTEM, 0, vec![]);
    t.set(vault_of(&STRANGER), SYSTEM, 5_000_000, vec![]);
}

fn scenarios() -> Vec<(&'static str, Vec<Instruction>)> {
    let accounts = (USER, vault_of(&USER), SYSTEM);
    let unsigned = {
        let mut ix = vault::withdraw::instruction(accounts, Amount { amount: 1 });
        ix.accounts[0].is_signer = false;
        ix
    };
    vec![
        ("deposit", vec![vault::deposit::instruction(accounts, Amount { amount: 1_000_000 })]),
        (
            "deposit then withdraw",
            vec![
                vault::deposit::instruction(accounts, Amount { amount: 1_000_000 }),
                vault::withdraw::instruction(accounts, Amount { amount: 100_000 }),
            ],
        ),
        (
            "withdraw more than deposited",
            vec![vault::withdraw::instruction(accounts, Amount { amount: 1 })],
        ),
        ("withdraw from another user's vault", {
            vec![vault::withdraw::instruction(
                (USER, vault_of(&STRANGER), SYSTEM),
                Amount { amount: 1 },
            )]
        }),
        ("withdraw without the user's signature", vec![unsigned]),
    ]
}

#[test]
fn every_scenario_matches_master() {
    for (name, instructions) in scenarios() {
        let mut ours = Test::new(ID, "vault");
        let mut theirs = Test::rival(ID, "blueshift_vault");
        prepare(&mut ours);
        prepare(&mut theirs);
        for instruction in instructions {
            let (a, b) = (ours.run(instruction.clone()).is_ok(), theirs.run(instruction).is_ok());
            assert_eq!(a, b, "{name}: we succeeded: {a}, Quasar master succeeded: {b}");
        }
        assert_eq!(ours.state(), theirs.state(), "{name}: account state differs");
    }
}
