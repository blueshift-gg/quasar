#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Data {
    pub value: PodU64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub authority: Signer,

    #[account(mut, close(dest = authority, extra = authority))]
    pub target: Account<Data>,
}

fn main() {}
