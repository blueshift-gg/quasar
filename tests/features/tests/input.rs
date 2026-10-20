//! The input adapter against the runtime's own serializer: whatever account
//! lists the runtime can produce, the program must read back what was sent.

use features::{Counter, ID, Tag};
use proptest::{prelude::*, test_runner::RngSeed};
use quasar_lang::{Address, ProgramError, client::AccountMeta, error::Error};
use quasar_test::Test;

#[derive(Clone, Debug)]
struct Sent {
    lamports: u64,
    system_owned: bool,
    data: Vec<u8>,
    is_signer: bool,
    is_writable: bool,
    /// Repeat the account at this earlier position instead of a new one.
    repeat: Option<usize>,
}

fn sent() -> impl Strategy<Value = Sent> {
    (0..10_000_000_000u64, any::<bool>(), prop::collection::vec(any::<u8>(), 0..40)).prop_flat_map(
        |(lamports, system_owned, data)| {
            (any::<bool>(), any::<bool>(), prop::option::weighted(0.2, 0..8usize)).prop_map(
                move |(is_signer, is_writable, repeat)| Sent {
                    lamports,
                    system_owned,
                    data: data.clone(),
                    is_signer,
                    is_writable,
                    repeat,
                },
            )
        },
    )
}

/// What the probe should report for one account.
fn expected_record(key: &Address, owner: &Address, sent: &Sent) -> Vec<u8> {
    let head = &sent.data[..sent.data.len().min(16)];
    [
        key.as_array().as_slice(),
        owner.as_array(),
        &sent.lamports.to_le_bytes(),
        &(sent.data.len() as u64).to_le_bytes(),
        &[u8::from(sent.is_signer), u8::from(sent.is_writable), 0],
        head,
    ]
    .concat()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x5155_4153),
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_probe_reads_back_every_account_it_was_sent(
        accounts in prop::collection::vec(sent(), 0..8),
        tag in any::<u64>(),
    ) {
        let mut t = Test::new(ID, "features");
        let mut ix = features::probe::instruction(Vec::new(), Tag { tag });
        let mut expected = tag.to_le_bytes().to_vec();
        let mut keys = Vec::new();
        let mut repeated = false;
        for account in &accounts {
            let key = match account.repeat.filter(|&earlier| earlier < keys.len()) {
                Some(earlier) => {
                    repeated = true;
                    keys[earlier]
                }
                None => {
                    let owner = if account.system_owned { Address::default() } else { Address::new_unique() };
                    let key = t.account_with(owner, account.lamports, account.data.clone());
                    expected.extend(expected_record(&key, &owner, account));
                    key
                }
            };
            keys.push(key);
            ix.accounts.push(AccountMeta { pubkey: key, is_signer: account.is_signer, is_writable: account.is_writable });
        }

        let outcome = t.run(ix);
        if repeated {
            outcome.fails(Error::DuplicateAccount);
        } else {
            let outcome = outcome.succeeds();
            prop_assert_eq!(outcome.return_data(), expected.as_slice());
        }
    }
}

#[test]
fn accounts_after_a_dynamic_one_are_found() {
    for between_len in [0, 1, 7, 8, 9, 100] {
        let mut t = Test::new(ID, "features");
        let counter = Counter { count: 0, authority: Address::default() };
        let first = t.account(&counter);
        let between = t.account_with(Address::default(), 1_000_000, vec![0xab; between_len]);
        let last = t.account(&counter);

        t.run(features::touch_both::instruction((first, between, last), ())).succeeds();

        assert_eq!(t.read::<Counter>(&first).count, 1);
        assert_eq!(t.read::<Counter>(&last).count, 1, "data length {between_len} between");
    }
}

#[test]
fn absent_optional_accounts_are_none() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);

    let last = t.wallet(1_000_000);

    assert_eq!(t.run(features::maybe::instruction((None, None, last), ())).return_data(), [0, 0]);
    assert_eq!(
        t.run(features::maybe::instruction((None, Some(user), last), ())).return_data(),
        [0, 1]
    );
    assert_eq!(
        t.run(features::maybe::instruction((Some(user), None, last), ())).return_data(),
        [1, 0]
    );
}

#[test]
fn a_present_optional_account_is_checked() {
    let mut t = Test::new(ID, "features");
    let user = t.wallet(1_000_000);
    let last = t.wallet(1_000_000);
    let mut ix = features::maybe::instruction((Some(user), None, last), ());
    ix.accounts[0].is_signer = false;
    t.run(ix).fails(ProgramError::MissingRequiredSignature);
}
