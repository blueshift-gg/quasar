//! On-chain unit tests written like plain Rust tests.
//!
//! Everything typed comes from the program itself: instructions from the
//! generated client, addresses from `#[seeds]`, state from `#[account]`.

use {
    crate::{
        cpi::{CloseAccountInstruction, InitializeInstruction},
        state::{SimpleAccount, SimpleAccountData},
    },
    quasar_lang::prelude::QuasarError,
    quasar_test::prelude::*,
};

#[quasar_test]
fn initialize_stores_typed_state(test: &mut Test) {
    let payer = test.add(Wallet::new());
    let (account, bump) = test.derive_pda_with_bump(SimpleAccount::seeds(&payer));

    test.send(InitializeInstruction { payer, value: 42 })
        .succeeds();

    let state = test.read::<SimpleAccount>(account);
    assert_eq!(state.authority, payer);
    assert_eq!(state.value, 42);
    assert_eq!(state.bump, bump);
}

#[quasar_test]
fn close_returns_the_account_to_the_system(test: &mut Test) {
    let authority = test.add(Wallet::new());
    let (account, bump) = test.derive_pda_with_bump(SimpleAccount::seeds(&authority));
    test.write(
        account,
        SimpleAccountData {
            authority,
            value: 7.into(),
            bump,
        },
    );

    test.send(CloseAccountInstruction { authority })
        .succeeds()
        .is_closed(account);
}

#[quasar_test]
fn close_rejects_a_foreign_authority(test: &mut Test) {
    let owner = test.add(Wallet::new());
    let intruder = test.add(Wallet::new());
    let (account, bump) = test.derive_pda_with_bump(SimpleAccount::seeds(&owner));
    test.write(
        account,
        SimpleAccountData {
            authority: owner,
            value: 7.into(),
            bump,
        },
    );

    // The in-crate client infers this account, so the negative test makes its
    // one adversarial mutation explicit.
    let intruder_pda = test.derive_pda(SimpleAccount::seeds(&intruder));
    let mut instruction: Instruction = CloseAccountInstruction {
        authority: intruder,
    }
    .into();
    instruction
        .accounts
        .iter_mut()
        .find(|meta| meta.pubkey == intruder_pda)
        .expect("generated account")
        .pubkey = account;

    test.send(instruction).fails_with(QuasarError::InvalidPda);
}
