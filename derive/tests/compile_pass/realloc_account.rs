//! Realloc via op dispatch.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 4)]
pub struct MyData {
    pub value: u64,
}

#[derive(Accounts)]
pub struct ReallocAccounts {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut,
        realloc = 200, payer = payer,
    )]
    pub data: Account<MyData>,

    pub system_program: Program<SystemProgram>,
}

fn main() {}
