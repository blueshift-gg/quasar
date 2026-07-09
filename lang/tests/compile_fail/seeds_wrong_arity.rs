#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"vault", authority: Address)]
pub struct Vault {
    pub authority: Address,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Bad {
    pub authority: Signer,
    // `Vault::seeds` takes the declared `authority: Address` seed. Calling it
    // with no arguments must fail — the typed seed constructor enforces arity.
    #[account(address = Vault::seeds())]
    pub vault: Account<Vault>,
}

fn main() {}
