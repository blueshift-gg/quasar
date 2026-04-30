//! Optional account with user checks.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 7)]
pub struct Config {
    pub authority: Address,
}

#[derive(Accounts)]
pub struct OptionalAccounts {
    pub authority: Signer,

    #[account(
        has_one(authority),
    )]
    pub config: Option<Account<Config>>,
}

fn main() {}
