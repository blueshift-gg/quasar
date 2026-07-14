#![allow(unexpected_cfgs)]
//! `dup` combined with a mutation op (`init`) is rejected: mutating an account
//! reachable through an alias is unsound.

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
    /// CHECK: intentionally aliased for the test; the mutation op is the error.
    #[account(dup, init, payer = payer)]
    pub aliased: Account<Data>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
