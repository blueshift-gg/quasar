#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"config")]
pub struct Config {
    pub namespace: u32,
    pub bump: u8,
}

#[account(discriminator = 2)]
#[seeds(b"item", namespace: u32)]
pub struct Item {
    pub namespace: u32,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Good {
    #[account(mut)]
    pub payer: Signer,
    pub config: Account<Config>,
    #[account(
        mut,
        init_if_needed,
        payer = payer,
        seeds = Item::seeds(config.namespace),
        bump
    )]
    pub item: Account<Item>,
    pub system_program: Program<System>,
}

fn main() {}
