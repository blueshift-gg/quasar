#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;
use quasar_spl::Mint;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BadCloseMint {
    #[account(mut)]
    pub destination: UncheckedAccount,
    #[account(mut, close = destination)]
    pub mint: Account<Mint>,
}

fn main() {}
