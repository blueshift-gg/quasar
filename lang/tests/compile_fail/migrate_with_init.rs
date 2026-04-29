#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub value: PodU64,
}

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub value: PodU64,
    pub extra: PodU32,
}

// ERROR: Migration cannot be combined with init
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(init, payer = payer)]
    pub config: Migration<ConfigV1, ConfigV2>,
}

fn main() {}
