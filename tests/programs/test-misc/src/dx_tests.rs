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
    quasar_test::{prelude::*, quasar_svm::system_program},
};

#[quasar_test]
fn initialize_stores_typed_state(q: &mut QuasarTest) {
    let payer = q.actor();
    let (account, bump) = q.pda_with_bump(SimpleAccount::seeds(&payer));

    q.send(InitializeInstruction {
        payer,
        account,
        system_program: system_program::ID,
        value: 42,
    })
    .succeeds();

    let state = q.read::<SimpleAccount>(account);
    assert_eq!(state.authority, payer);
    assert_eq!(state.value, 42);
    assert_eq!(state.bump, bump);
}

#[quasar_test]
fn close_returns_the_account_to_the_system(q: &mut QuasarTest) {
    let authority = q.actor();
    let (account, bump) = q.pda_with_bump(SimpleAccount::seeds(&authority));
    q.write::<SimpleAccount>(
        account,
        SimpleAccountData {
            authority,
            value: 7.into(),
            bump,
        },
    );

    q.send(CloseAccountInstruction { authority, account })
        .succeeds()
        .is_closed(account);
}

#[quasar_test]
fn close_rejects_a_foreign_authority(q: &mut QuasarTest) {
    let [owner, intruder] = q.actors();
    let (account, bump) = q.pda_with_bump(SimpleAccount::seeds(&owner));
    q.write::<SimpleAccount>(
        account,
        SimpleAccountData {
            authority: owner,
            value: 7.into(),
            bump,
        },
    );

    q.send(CloseAccountInstruction {
        authority: intruder,
        account,
    })
    .fails_with(QuasarError::InvalidPda as u32);
}
