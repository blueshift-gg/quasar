//! User checks: has_one, address, constraints.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 3)]
pub struct Vault {
    pub authority: Address,
    pub amount: u64,
}

#[derive(Accounts)]
pub struct CheckAccounts {
    pub authority: Signer,

    #[account(
        has_one(authority),
        constraints(vault.amount > 0),
    )]
    pub vault: Account<Vault>,
}

fn main() {}
