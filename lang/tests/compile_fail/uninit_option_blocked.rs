#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Vault {
    pub value: PodU64,
}

// ERROR: Option<Uninit<...>> is not supported
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,

    pub vault: Option<Uninit<Account<Vault>>>,
}

fn main() {}
