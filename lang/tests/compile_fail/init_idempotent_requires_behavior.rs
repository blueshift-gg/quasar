#![allow(unexpected_cfgs)]
//! `init(idempotent)` needs a behavior group or an address constraint to decide
//! whether the account already exists; a bare idempotent init is rejected.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    #[account(init(idempotent), payer = payer)]
    pub data: Account<Data>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
