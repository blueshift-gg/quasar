#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct Data {
    pub value: u64,
}

// Three independent field violations must all surface in a single compile
// cycle (syn::Error::combine), not one-at-a-time across three edit/compile
// rounds.
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,

    // Violation 1: `realloc` on a `Signer` (structural-op type gate).
    #[account(mut, realloc = 64)]
    pub sig: Signer,

    // Violation 2: `init` on an `Option<T>` field.
    #[account(init, payer = payer)]
    pub maybe: Option<Account<Data>>,

    // Violation 3: `dup` without a `/// CHECK:` doc comment.
    #[account(dup)]
    pub aliased: UncheckedAccount,

    pub system_program: Program<SystemProgram>,
}

fn main() {}
