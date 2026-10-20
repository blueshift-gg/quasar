//! The multisig against public Quasar master's multisig, built from a pinned
//! revision by `bench/rivals.sh`. The oracle compares outcomes and the vault's
//! and recipient's balances without requiring identical configuration storage.

use multisig::{Amount, ID, Label, MultisigConfig, Threshold};
use quasar_lang::sysvar::{Rent, SysvarData};
use quasar_lang::{Address, ProgramError, client::Instruction, error::Error, zeropod};
use quasar_test::Test;

const SYSTEM: Address = Address::new_from_array([0; 32]);
const CREATOR: Address = Address::new_from_array([1; 32]);
const RECIPIENT: Address = Address::new_from_array([2; 32]);

fn member(index: u8) -> Address {
    Address::new_from_array([10 + index; 32])
}

fn config() -> Address {
    Address::find_program_address(&[b"multisig", CREATOR.as_ref()], &ID).0
}

fn vault() -> Address {
    Address::find_program_address(&[b"vault", config().as_ref()], &ID).0
}

fn prepare(mut t: Test) -> Test {
    t.set(CREATOR, SYSTEM, 10_000_000_000, vec![]);
    t.set(RECIPIENT, SYSTEM, 1_000_000, vec![]);
    for index in 0..12 {
        t.set(member(index), SYSTEM, 1_000_000, vec![]);
    }
    t.set(config(), SYSTEM, 0, vec![]);
    t.set(vault(), SYSTEM, 0, vec![]);
    t
}

/// A multisig of `members` members, `threshold` of them needed.
fn create(members: u8, threshold: u8) -> Instruction {
    let members = (0..members).map(member).collect();
    multisig::create::instruction(
        (CREATOR, config(), Rent::ID, SYSTEM, members),
        Threshold { threshold },
    )
}

fn deposit(amount: u64) -> Instruction {
    multisig::deposit::instruction((CREATOR, config(), vault(), SYSTEM), Amount { amount })
}

fn execute(approvals: &[Address], amount: u64) -> Instruction {
    let accounts = (config(), CREATOR, vault(), RECIPIENT, SYSTEM, approvals.to_vec());
    multisig::execute_transfer::instruction(accounts, Amount { amount })
}

fn members(indices: impl IntoIterator<Item = u8>) -> Vec<Address> {
    indices.into_iter().map(member).collect()
}

#[test]
fn a_threshold_of_members_can_pay_out() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(3, 2)).succeeds();
    t.run(deposit(5_000)).succeeds();

    t.run(execute(&members([0, 2]), 3_000)).succeeds();

    assert_eq!(t.lamports(&RECIPIENT), 1_003_000);
    let stored = t.read::<MultisigConfig>(&config());
    assert_eq!((stored.threshold, stored.members), (2, members(0..3)));
}

#[test]
fn fewer_approvals_than_the_threshold_are_refused() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(3, 2)).succeeds();
    t.run(deposit(5_000)).succeeds();

    t.run(execute(&members([1]), 1)).fails(ProgramError::MissingRequiredSignature);
    t.run(execute(&[], 1)).fails(ProgramError::MissingRequiredSignature);
    // Signers who are not members do not count.
    t.run(execute(&members([1, 5, 6]), 1)).fails(ProgramError::MissingRequiredSignature);
}

#[test]
fn a_member_cannot_approve_twice() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(3, 2)).succeeds();
    t.run(deposit(5_000)).succeeds();

    t.run(execute(&members([1, 1]), 1)).fails(Error::DuplicateAccount);
}

#[test]
fn members_and_approvals_have_a_maximum() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(11, 2)).fails(ProgramError::InvalidArgument);
    t.run(create(10, 11)).fails(ProgramError::InvalidArgument);
    t.run(create(0, 1)).fails(ProgramError::InvalidArgument);

    t.run(create(10, 2)).succeeds();
    t.run(execute(&members(0..11), 1)).fails(ProgramError::InvalidArgument);
}

fn set_label(signer: Address, label: &str) -> Instruction {
    let label = zeropod::ArrayString::<32>::try_from(label).unwrap();
    multisig::set_label::instruction((signer, config(), SYSTEM), Label { label })
}

fn label(t: &Test) -> String {
    t.read::<MultisigConfig>(&config()).label
}

#[test]
fn only_the_creator_labels_the_multisig() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(3, 2)).succeeds();

    t.run(set_label(CREATOR, "treasury")).succeeds();
    assert_eq!(label(&t), "treasury");

    t.run(set_label(member(0), "stolen")).fails(Error::HasOne);
}

#[test]
fn the_configuration_grows_and_shrinks_with_its_label_and_stays_rent_exempt() {
    let mut t = prepare(Test::new(ID, "multisig"));
    t.run(create(3, 2)).succeeds();
    let rent_exempt = |t: &Test| t.lamports(&config()) == t.rent(t.data(&config()).len());
    let before = (t.data(&config()).len(), t.lamports(&CREATOR));

    // The creator pays for the eight bytes the label adds.
    t.run(set_label(CREATOR, "treasury")).succeeds();
    assert_eq!(t.data(&config()).len(), before.0 + 8);
    assert!(rent_exempt(&t));

    // Emptied, the label's bytes go, and their rent comes back to the creator.
    t.run(set_label(CREATOR, "")).succeeds();
    assert_eq!((label(&t), t.data(&config()).len()), (String::new(), before.0));
    assert!(rent_exempt(&t));
    assert_eq!(t.lamports(&CREATOR), before.1);
}

fn scenarios() -> Vec<(&'static str, Vec<Instruction>)> {
    let paid = |approvals: &[u8]| {
        vec![create(3, 2), deposit(5_000), execute(&members(approvals.iter().copied()), 3_000)]
    };
    vec![
        ("create", vec![create(3, 2)]),
        ("two of three approve", paid(&[0, 2])),
        ("one of three approves", paid(&[1])),
        ("no one approves", paid(&[])),
        ("strangers approve", paid(&[5, 6])),
        ("one member approves twice", paid(&[1, 1])),
        ("eleven members", vec![create(11, 2)]),
        ("a threshold of zero", vec![create(3, 0)]),
        ("a threshold above the members", vec![create(3, 4)]),
        ("execute with nothing created", vec![execute(&members([0, 1]), 1)]),
    ]
}

#[test]
fn every_scenario_matches_master() {
    for (name, instructions) in scenarios() {
        let mut ours = prepare(Test::new(ID, "multisig"));
        let mut theirs = prepare(Test::rival(ID, "quasar_multisig"));
        for instruction in instructions {
            let (a, b) = (ours.run(instruction.clone()).is_ok(), theirs.run(instruction).is_ok());
            assert_eq!(a, b, "{name}: we succeeded: {a}, Quasar master succeeded: {b}");
        }
        for account in [vault(), RECIPIENT] {
            let (a, b) = (ours.lamports(&account), theirs.lamports(&account));
            assert_eq!(a, b, "{name}: {account} holds {a} here, {b} in Quasar master");
        }
    }
}

#[test]
fn the_idl_matches_the_committed_one() {
    let idl = multisig::idl().to_json();
    quasar_test::snapshot(concat!(env!("CARGO_MANIFEST_DIR"), "/idl.json"), &idl);
}
