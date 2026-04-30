//! Exit close via close_program::Op.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_lang::ops::close_program;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 6)]
pub struct OldData {
    pub value: u64,
}

#[derive(Accounts)]
pub struct CloseAccounts {
    #[account(mut)]
    pub authority: Signer,

    #[account(mut,
        close_program(dest = authority),
    )]
    pub old_data: Account<OldData>,
}

fn main() {}
