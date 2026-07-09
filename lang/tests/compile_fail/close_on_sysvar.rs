#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ERROR: `close` requires a program data account (`Account<T>` /
// `InterfaceAccount<T>`), not a `Sysvar`. The type gate must reject this at the
// field with one spanned error.
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    #[account(close(dest = payer))]
    pub clock: Sysvar<Clock>,
}

fn main() {}
