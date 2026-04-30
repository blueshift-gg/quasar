#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct DemoAccount {
    pub value: u64,
}

#[derive(Accounts)]
pub struct BadReallocOptional {
    #[account(mut, realloc = 64)]
    pub account: Option<Account<DemoAccount>>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
