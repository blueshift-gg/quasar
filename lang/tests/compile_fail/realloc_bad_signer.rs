#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BadReallocSigner {
    #[account(mut, realloc = 64)]
    pub account: Signer,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
