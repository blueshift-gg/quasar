//! Multiple account types in one struct.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 8)]
pub struct Config {
    pub authority: Address,
    pub value: u64,
}

#[derive(Accounts)]
pub struct MultiAccounts {
    #[account(mut)]
    pub payer: Signer,

    pub config: Account<Config>,

    pub recipient: SystemAccount,

    pub system_program: Program<SystemProgram>,
}

fn main() {}
