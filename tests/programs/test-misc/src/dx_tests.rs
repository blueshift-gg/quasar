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
fn initialize_stores_typed_state() {
    let payer = ctx.add(Wallet::account());
    let (account, bump) = ctx.derive_pda_with_bump(SimpleAccount::seeds(&payer));

    ctx.execute(InitializeInstruction { payer, value: 42 })
        .check(Outcome::success());

    let state = ctx.read::<SimpleAccount>(account);
    assert_eq!(state.authority, payer);
    assert_eq!(state.value, 42);
    assert_eq!(state.bump, bump);
}

#[quasar_test]
fn close_returns_the_account_to_the_system() {
    let authority = ctx.add(Wallet::account());
    let (account, bump) = ctx.derive_pda_with_bump(SimpleAccount::seeds(&authority));
    ctx.write(
        account,
        SimpleAccountData {
            authority,
            value: 7.into(),
            bump,
        },
    );

    ctx.execute(CloseAccountInstruction { authority })
        .check(Outcome::success())
        .check(Account::closed(account));
}

#[quasar_test]
fn close_rejects_a_foreign_authority() {
    let owner = ctx.add(Wallet::account());
    let intruder = ctx.add(Wallet::account());
    let (account, bump) = ctx.derive_pda_with_bump(SimpleAccount::seeds(&owner));
    ctx.write(
        account,
        SimpleAccountData {
            authority: owner,
            value: 7.into(),
            bump,
        },
    );

    // The in-crate client infers this account, so the negative test makes its
    // one adversarial mutation explicit.
    let intruder_pda = ctx.derive_pda(SimpleAccount::seeds(&intruder));
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

    ctx.execute(instruction)
        .check(Outcome::error(QuasarError::InvalidPda));
}
