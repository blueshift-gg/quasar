#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub value: PodU64,
}

// ERROR: migration source must be a program account type, not Signer.
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,

    #[account(mut, payer = payer)]
    pub target: Migration<Signer, ConfigV2>,
}

fn main() {}
