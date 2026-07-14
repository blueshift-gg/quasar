#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Vault {
    pub authority: Address,
    pub bump: u8,
}

// Anchor's `seeds = [...]` / `bump` PDA syntax is not supported. The derive must
// redirect to the typed-seeds form: `#[seeds(...)]` on the account type plus
// `#[account(address = Vault::seeds(<args>))]` on the field, with the bump
// derived and stored automatically.
#[derive(Accounts)]
pub struct Bad {
    pub authority: Signer,
    #[account(seeds = [b"vault", authority], bump)]
    pub vault: Account<Vault>,
}

fn main() {}
