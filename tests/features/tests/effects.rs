use features::{Amount, ID, Len, Vault};
use quasar_lang::{Address, ProgramError, account::Kind, zeropod};
use quasar_test::{InstructionError, Test};

/// A vault holding its rent-exempt balance and `spare` lamports more.
fn vault(t: &mut Test, spare: u64) -> Address {
    let data = [
        Vault::DISCRIMINATOR,
        &zeropod::to_vec(&Vault { depositor: Address::default(), bump: 0, amount: 0 }).unwrap(),
    ]
    .concat();
    t.account_with(ID, t.rent(data.len()) + spare, data)
}

#[test]
fn lamports_move_directly_out_of_a_program_owned_account_and_an_event_says_so() {
    let mut t = Test::new(ID, "features");
    let vault = vault(&mut t, 500);
    let to = t.wallet(1_000_000);
    let before = t.lamports(&vault);

    let outcome = t.run(features::pay::instruction((vault, to), Amount { amount: 300 })).succeeds();

    assert_eq!(t.lamports(&vault), before - 300);
    assert_eq!(t.lamports(&to), 1_000_300);
    // The event: its discriminator, then its fields' bytes.
    let paid = [&[9][..], &300u64.to_le_bytes(), to.as_ref()].concat();
    assert_eq!(outcome.events(), [paid]);
}

#[test]
fn moving_more_lamports_than_the_account_holds_fails() {
    let mut t = Test::new(ID, "features");
    let vault = vault(&mut t, 0);
    let to = t.wallet(1_000_000);
    let too_much = t.lamports(&vault) + 1;

    t.run(features::pay::instruction((vault, to), Amount { amount: too_much }))
        .fails(ProgramError::InsufficientFunds);
}

#[test]
fn a_closed_account_is_empty_and_its_lamports_are_paid_out() {
    let mut t = Test::new(ID, "features");
    let vault = vault(&mut t, 500);
    let to = t.wallet(1_000_000);
    let balance = t.lamports(&vault);

    t.run(features::close::instruction((vault, to), ())).succeeds();

    assert_eq!(t.lamports(&to), 1_000_000 + balance);
    assert_eq!(t.lamports(&vault), 0);
    assert!(t.data(&vault).is_empty());
    assert_eq!(t.owner(&vault), Address::default());
}

#[test]
fn an_account_grows_with_zeroed_bytes_and_shrinks() {
    let mut t = Test::new(ID, "features");
    let data = t.account_with(ID, t.rent(100), vec![7; 10]);

    t.run(features::resize::instruction(data, Len { len: 100 })).succeeds();
    assert_eq!(t.data(&data), [[7; 10].as_slice(), &[0; 90]].concat());

    t.run(features::resize::instruction(data, Len { len: 5 })).succeeds();
    assert_eq!(t.data(&data), [7; 5]);
}

#[test]
fn growing_past_the_realloc_reserve_fails() {
    let mut t = Test::new(ID, "features");
    let data = t.account_with(ID, t.rent(20_000), vec![0; 10]);

    t.run(features::resize::instruction(data, Len { len: 10 + 10 * 1024 + 1 }))
        .fails(ProgramError::InvalidRealloc);
}

#[test]
fn only_the_owning_program_can_take_lamports_out() {
    let mut t = Test::new(ID, "features");
    let owned = t.set(Address::new_unique(), ID, 1_000_000, vec![]);
    let wallet = t.wallet(1_000_000);
    let to = t.wallet(0);

    t.run(features::drain::instruction((owned, to), Amount { amount: 400 })).succeeds();
    t.run(features::drain::instruction((wallet, to), Amount { amount: 400 }))
        .rejected(InstructionError::ExternalAccountLamportSpend);

    assert_eq!(t.lamports(&to), 400);
}
