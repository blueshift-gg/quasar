use callee::{Purse, Tally};
use features::{Amount, Counter, ID, Seeded, Seeding, Vault};
use quasar_lang::{Address, ProgramError, error::Error};
use quasar_test::{InstructionError, Test};

/// A counter's data: a one-byte discriminator, a `u64` and an address.
const COUNTER_LEN: usize = 1 + 8 + 32;

fn with_callee() -> Test {
    let mut t = Test::new(ID, "features");
    t.add_program(callee::ID, "callee");
    t
}

#[test]
fn the_callee_receives_the_accounts_flags_and_data_it_was_given() {
    let mut t = with_callee();
    let record = t.account_with(callee::ID, t.rent(76), vec![0; 76]);
    let user = t.wallet(1_000_000);
    let signer = Address::find_program_address(&[b"signer"], &ID).0;
    let pda = t.set(signer, Address::default(), 0, vec![]);

    t.run(features::call_record::instruction(
        (callee::ID, record, user, pda),
        callee::Payload { payload: 42 },
    ))
    .succeeds();

    let expected = [
        42u64.to_le_bytes().as_slice(),
        user.as_ref(),
        &[1, 0], // signer, read-only
        pda.as_ref(),
        &[1, 0], // signed for by the program through its seeds
    ]
    .concat();
    assert_eq!(t.data(&record), expected);
}

#[test]
fn a_callee_owned_account_is_checked_again_with_the_callees_change() {
    let mut t = with_callee();
    let tally = t.account(&Tally { count: 41 });

    let outcome = t.run(features::call_bump::instruction((callee::ID, tally), ())).succeeds();

    assert_eq!(outcome.return_data(), 42u64.to_le_bytes());
}

/// `call_touch` leaves the purse's checks to the callee, which makes them:
/// what would not have loaded here fails the call.
#[test]
fn checks_left_to_a_callee_are_made_by_it() {
    let mut t = with_callee();
    let (holder, other) = (t.wallet(1_000_000), t.wallet(1_000_000));
    let purse = t.account(&Purse { holder, count: 7 });
    let not_a_purse = t.account(&Tally { count: 7 });
    let forged = t.account_with(ID, t.rent(41), t.data(&purse).to_vec());
    let touch = |holder, purse| features::call_touch::instruction((callee::ID, holder, purse), ());

    let before = t.state();
    assert!(!t.run(touch(other, purse)).is_ok(), "not its holder");
    assert!(!t.run(touch(holder, not_a_purse)).is_ok(), "another kind");
    assert!(!t.run(touch(holder, forged)).is_ok(), "another program's account");
    assert_eq!(t.state(), before);

    // Built to make every check itself, the program decides the same.
    let mut checked = Test::debug(ID, "features");
    checked.add_program(callee::ID, "callee");
    for key in [holder, other, purse, not_a_purse, forged] {
        checked.set(key, t.owner(&key), t.lamports(&key), t.data(&key).to_vec());
    }
    for (holder, purse) in
        [(other, purse), (holder, not_a_purse), (holder, forged), (holder, purse)]
    {
        let (elided, every) = (t.run(touch(holder, purse)), checked.run(touch(holder, purse)));
        assert_eq!(elided.is_ok(), every.is_ok());
    }
    assert_eq!(t.data(&purse), checked.data(&purse));
    assert_eq!(t.read::<Purse>(&purse).count, 8);
    assert_eq!(
        features::elisions(),
        "call_bump.tally: callee::Tally\n\
         call_self.vault: features::Vault\n\
         read_after_call.vault: features::Vault\n\
         reload_after_call.vault: features::Vault\n\
         call_touch.purse: callee::Purse\n\
         call_touch.purse: has_one(holder)\n"
    );
}

/// Built with `quasar-lang/debug`, the program says which account failed
/// which check, as the accounts struct writes it, before it fails.
#[test]
fn the_debug_build_says_what_failed() {
    let mut t = Test::debug(ID, "features");
    t.add_program(callee::ID, "callee");
    let (holder, other) = (t.wallet(1_000_000), t.wallet(1_000_000));
    let purse = t.account(&Purse { holder, count: 7 });
    let tally = t.account(&Tally { count: 7 });
    let touch = |holder, purse| features::call_touch::instruction((callee::ID, holder, purse), ());
    let said = |outcome: quasar_test::Outcome, what: &str| {
        assert!(outcome.logs().iter().any(|line| line.contains(what)), "{:?}", outcome.logs());
    };

    said(t.run(touch(other, purse)), "error[HasOne]: account `purse` stores `holder` = ");
    said(
        t.run(touch(holder, tally)),
        "error[DataLength]: account `purse` has 9 bytes of data, expected 41",
    );
    let mut unsigned = touch(holder, purse);
    unsigned.accounts[1].is_signer = false;
    said(
        t.run(unsigned),
        "error[MissingRequiredSignature]: account `holder` did not sign the transaction",
    );

    let vault = t.account(&Vault { depositor: holder, bump: 0, amount: 0 });
    let read = features::read_after_call::instruction((ID, vault, other), ());
    said(t.run(read), "call `.reload()?` on it right after that call");
}

#[test]
fn an_account_this_program_owns_stays_usable_across_a_call() {
    let mut t = with_callee();
    let counter = t.account(&Counter { count: 1, authority: Address::default() });

    t.run(features::call_keeping_own::instruction((callee::ID, counter), ())).succeeds();

    assert_eq!(t.read::<Counter>(&counter).count, 2);
}

#[test]
fn a_created_account_is_rent_exempt_and_holds_its_kind() {
    let mut t = Test::new(ID, "features");
    let payer = t.wallet(1_000_000_000);
    let (address, bump) = Address::find_program_address(&[b"counter", payer.as_ref()], &ID);
    let counter = t.set(address, Address::default(), 0, vec![]);
    let system = Address::default();

    t.run(features::create_counter::instruction((payer, counter, system), ())).succeeds();

    let created = t.read::<Counter>(&counter);
    assert_eq!((created.count, created.authority), (u64::from(bump), payer));
    assert_eq!(t.lamports(&counter), t.rent(COUNTER_LEN));
}

/// Anyone can send lamports to an address before it is created.
#[test]
fn an_account_already_holding_lamports_is_still_created() {
    let mut t = Test::new(ID, "features");
    let payer = t.wallet(1_000_000_000);
    let (address, _) = Address::find_program_address(&[b"counter", payer.as_ref()], &ID);
    let rent = t.rent(COUNTER_LEN);

    for held in [1, rent + 1] {
        let counter = t.set(address, Address::default(), held, vec![]);

        t.run(features::create_counter::instruction((payer, counter, Address::default()), ()))
            .succeeds();

        assert_eq!(t.read::<Counter>(&counter).authority, payer);
        assert_eq!(t.lamports(&counter), rent.max(held));
    }
}

#[test]
fn creating_at_an_address_that_is_not_the_pda_fails() {
    let mut t = Test::new(ID, "features");
    let payer = t.wallet(1_000_000_000);
    let counter = t.wallet(0);

    t.run(features::create_counter::instruction((payer, counter, Address::default()), ()))
        .fails(ProgramError::InvalidSeeds);
}

#[test]
fn lamports_move_from_a_signer_through_the_system_program() {
    let mut t = Test::new(ID, "features");
    let from = t.wallet(1_000_000);
    let to = t.wallet(1_000_000);

    t.run(features::fund::instruction((from, to, Address::default()), Amount { amount: 400 }))
        .succeeds();

    assert_eq!((t.lamports(&from), t.lamports(&to)), (999_600, 1_000_400));
}

#[test]
fn an_account_a_call_may_have_changed_is_read_only_after_reload() {
    let mut t = Test::new(ID, "features");
    let depositor = Address::new_unique();
    let vault = t.account(&Vault { depositor, bump: 0, amount: 0 });
    let to = t.wallet(0);
    let accounts = (ID, vault, to);

    // Read without `reload()`, the program ends at the read, an access
    // violation; reloaded, the closed vault fails its checks.
    t.run(features::read_after_call::instruction(accounts, ()))
        .rejected(InstructionError::ProgramFailedToComplete);
    // A test that expected success is told why.
    let unexpected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.run(features::read_after_call::instruction(accounts, ())).succeeds();
    }));
    let message = unexpected.unwrap_err().downcast::<String>().unwrap();
    assert!(message.contains("call `.reload()?` on it right after that call"), "{message}");
    assert!(message.contains(&vault.to_string()), "{message}");
    t.run(features::reload_after_call::instruction(accounts, ())).fails(Error::DataLength);
    // Not read again, it needs neither.
    t.run(features::call_self::instruction(accounts, ())).succeeds();
    assert_eq!(t.owner(&vault), Address::default());
}

#[test]
fn init_without_seeds_creates_a_signing_account_once() {
    let mut t = Test::new(ID, "features");
    let payer = t.wallet(1_000_000_000);
    let note = t.wallet(0);
    let create = features::create_note::instruction((payer, note, Address::default()), ());

    t.run(create.clone()).succeeds();
    t.run(create).succeeds(); // idempotent: the account already holds a counter

    assert_eq!((t.owner(&note), t.data(&note).len()), (ID, COUNTER_LEN));
    assert_eq!(t.lamports(&note), t.rent(COUNTER_LEN));

    let mut unsigned =
        features::create_note::instruction((payer, t.wallet(0), Address::default()), ());
    unsigned.accounts[1].is_signer = false;
    t.run(unsigned).fails(ProgramError::MissingRequiredSignature);
}

#[test]
fn an_accounts_struct_derives_an_address_from_an_instruction_argument() {
    let mut t = Test::new(ID, "features");
    let creator = t.wallet(1_000_000_000);
    let at = |seed: u64| {
        Address::find_program_address(&[b"seeded", creator.as_ref(), &seed.to_le_bytes()], &ID).0
    };
    let seeded = t.set(at(7), Address::default(), 0, vec![]);

    t.run(features::create_seeded::instruction(
        (creator, seeded, Address::default()),
        Seeding { seed: 8 },
    ))
    .fails(ProgramError::InvalidSeeds);
    t.run(features::create_seeded::instruction(
        (creator, seeded, Address::default()),
        Seeding { seed: 7 },
    ))
    .succeeds();

    assert_eq!(t.read::<Seeded>(&seeded).seed, 7);
}
