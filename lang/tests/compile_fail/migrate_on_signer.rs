#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub value: PodU64,
}

// ERROR: #[account(migrate = X)] is deprecated
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,

    #[account(migrate = ConfigV2, payer = payer)]
    pub target: Signer,
}

fn main() {}
