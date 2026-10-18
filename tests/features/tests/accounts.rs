use features::{Amount, By, Counter, ID, Profile, VaultV1, VaultV2};
use quasar_lang::{Address, ProgramError, account::Kind, error::Error, zeropod};
use quasar_test::Test;

fn counter(count: u64) -> Counter {
    Counter { count, authority: Address::new_from_array([1; 32]) }
}

/// The bytes a valid counter account holds.
fn counter_data() -> Vec<u8> {
    [Counter::DISCRIMINATOR, &zeropod::to_vec(&counter(0)).unwrap()].concat()
}

#[test]
fn an_account_owned_by_another_program_is_rejected() {
    let mut t = Test::new(ID, "features");
    let data = counter_data();
    let key = t.account_with(Address::new_unique(), t.rent(data.len()), data);

    t.run(features::increment::instruction(key, By { by: 1 })).fails(ProgramError::IllegalOwner);
}

#[test]
fn an_account_of_the_wrong_length_is_rejected() {
    let mut t = Test::new(ID, "features");
    let mut data = counter_data();
    data.push(0);
    let key = t.account_with(ID, t.rent(data.len()), data);

    t.run(features::increment::instruction(key, By { by: 1 })).fails(Error::DataLength);
}

#[test]
fn an_account_with_another_discriminator_is_rejected() {
    let mut t = Test::new(ID, "features");
    let mut data = counter_data();
    data[0] = 2;
    let key = t.account_with(ID, t.rent(data.len()), data);

    t.run(features::increment::instruction(key, By { by: 1 })).fails(Error::Discriminator);
}

#[test]
fn a_read_only_account_is_rejected_where_mut_is_declared() {
    let mut t = Test::new(ID, "features");
    let key = t.account(&counter(0));
    let mut ix = features::increment::instruction(key, By { by: 1 });
    ix.accounts[0].is_writable = false;

    t.run(ix).fails(ProgramError::Immutable);
}

#[test]
fn a_program_owned_account_is_written_in_place_as_its_discriminator_then_its_fields() {
    let mut t = Test::new(ID, "features");
    let key = t.account(&counter(40));

    t.run(features::increment::instruction(key, By { by: 2 })).succeeds();

    let expected = [[1].as_slice(), &42u64.to_le_bytes(), &[1; 32]].concat();
    assert_eq!(t.data(&key), expected);
}

#[test]
fn a_system_account_must_be_owned_by_the_system_program() {
    let mut t = Test::new(ID, "features");
    let wallet = t.wallet(1_000_000);
    let counter = t.account(&counter(0));

    t.run(features::wallet::instruction(wallet, ())).succeeds();
    t.run(features::wallet::instruction(counter, ())).fails(ProgramError::IllegalOwner);
}

#[test]
fn a_call_reaches_only_its_program() {
    let mut t = Test::new(ID, "features");
    t.add_program(callee::ID, "callee");
    let from = t.wallet(1_000_000);
    let to = t.wallet(0);
    let not_executable = t.wallet(0);

    t.run(features::fund::instruction((from, to, not_executable), Amount { amount: 1 }))
        .fails(Error::NotExecutable);
    // Another program in its place is not called: the call names the System
    // Program, which the runtime does not find among the accounts.
    t.run(features::fund::instruction((from, to, callee::ID), Amount { amount: 1 }))
        .fails(ProgramError::NotEnoughAccountKeys);
    assert_eq!(t.lamports(&to), 0);
}

#[test]
fn remaining_accounts_are_each_checked_as_declared() {
    let mut t = Test::new(ID, "features");
    let signers = vec![t.wallet(1_000_000), t.wallet(1_000_000)];

    assert_eq!(t.run(features::signers::instruction(signers.clone(), ())).return_data(), [2]);

    let mut unsigned = features::signers::instruction(signers.clone(), ());
    unsigned.accounts[1].is_signer = false;
    t.run(unsigned).fails(ProgramError::MissingRequiredSignature);

    let mut read_only = features::signers::instruction(signers, ());
    read_only.accounts[1].is_writable = false;
    t.run(read_only).fails(ProgramError::Immutable);
}

#[test]
fn sysvars_read_in_place_and_by_syscall_match_the_runtime() {
    let mut t = Test::new(ID, "features");
    t.runtime().sysvars.warp_to_slot(1_234);
    let clock = t.runtime().sysvars.clock.clone();
    let accounts = [
        t.runtime().sysvars.keyed_account_for_clock_sysvar(),
        t.runtime().sysvars.keyed_account_for_rent_sysvar(),
    ];
    let [clock_key, rent_key] =
        accounts.map(|(key, account)| t.set(key, account.owner, account.lamports, account.data));

    let report = t
        .run(features::sysvars::instruction((clock_key, rent_key), ()))
        .succeeds()
        .return_data()
        .to_vec();

    let expected = [
        clock.slot.to_le_bytes(),
        clock.epoch_start_timestamp.to_le_bytes(),
        clock.epoch.to_le_bytes(),
        clock.leader_schedule_epoch.to_le_bytes(),
        clock.unix_timestamp.to_le_bytes(),
    ]
    .concat();
    assert_eq!(report[..40], expected);
    assert_eq!(report[40], 1, "`get` reads what the accounts hold");
    assert_eq!(report[41..], t.rent(41).to_le_bytes());

    let not_the_clock = t.account_with(Address::default(), 1_000_000, vec![0; 40]);
    t.run(features::sysvars::instruction((not_the_clock, rent_key), ())).fails(Error::Sysvar);
}

#[test]
fn strings_and_vectors_take_the_room_they_use_and_the_payer_its_rent() {
    let mut t = Test::new(ID, "features");
    let owner = t.wallet(1_000_000_000);
    let profile = t.wallet(0);

    t.run(features::create_profile::instruction((owner, profile, Address::default()), ()))
        .succeeds();

    let stored = t.read::<Profile>(&profile);
    assert_eq!((stored.name.as_str(), stored.members.as_slice()), ("quasar", [owner].as_slice()));
    // The discriminator, then Borsh's four-byte count before the six bytes of
    // name, and before the one member.
    assert_eq!(t.data(&profile).len(), 1 + (4 + 6) + (4 + 32));
    assert_eq!(t.lamports(&profile), t.rent(1 + (4 + 6) + (4 + 32)));
}

#[test]
fn an_old_layout_becomes_the_new_one_on_first_use() {
    let mut t = Test::new(ID, "features");
    let payer = t.wallet(1_000_000_000);
    let depositor = Address::new_unique();
    let vault = t.account(&VaultV1 { depositor, amount: 7 });
    let (payer_before, vault_before) = (t.lamports(&payer), t.lamports(&vault));

    let read =
        t.run(features::read_v2::instruction((payer, vault, Address::default()), ())).succeeds();

    assert_eq!(read.return_data(), 7u64.to_le_bytes());
    let migrated = t.read::<VaultV2>(&vault);
    assert_eq!((migrated.depositor, migrated.amount, migrated.locked_until), (depositor, 7, 0));
    let len = quasar_lang::account::len::<VaultV2>().unwrap();
    assert_eq!(t.data(&vault).len(), len);
    assert_eq!(t.lamports(&vault), t.rent(len));
    assert_eq!(payer_before - t.lamports(&payer), t.rent(len) - vault_before);

    // Already migrated: loaded as it is, and nothing is paid.
    let payer_before = t.lamports(&payer);
    t.run(features::read_v2::instruction((payer, vault, Address::default()), ())).succeeds();
    assert_eq!(t.lamports(&payer), payer_before);
}
