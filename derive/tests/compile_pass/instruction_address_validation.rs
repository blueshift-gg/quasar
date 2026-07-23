#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_derive::Accounts;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"vault", authority: Address)]
pub struct Vault {
    pub authority: Address,
}

#[derive(Accounts)]
#[instruction(authority: Address)]
pub struct ValidateAddressArg {
    #[account(
        address = Vault::seeds(authority),
        constraints(vault.authority == authority),
    )]
    pub vault: Account<Vault>,
}

fn main() {}
