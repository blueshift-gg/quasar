#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BadReallocUnchecked {
    #[account(mut, realloc = 64)]
    pub account: UncheckedAccount,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
