//! Plain accounts — Signer, Account, Program, SystemAccount.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct TestConfig {
    pub value: u64,
}

#[derive(Accounts)]
pub struct BasicAccounts {
    #[account(mut)]
    pub payer: Signer,

    pub config: Account<TestConfig>,

    pub system_program: Program<SystemProgram>,
}

fn main() {}
