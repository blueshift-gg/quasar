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

impl Config {
    pub fn namespace_seed_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                &self.namespace as *const _ as *const u8,
                core::mem::size_of_val(&self.namespace),
            )
        }
    }
}

#[account(discriminator = 2)]
#[seeds(b"item", namespace_bytes: [u8; 4])]
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
        init,
        payer = payer,
        seeds = Item::seeds(config.namespace_seed_bytes()),
        bump
    )]
    pub item: Account<Item>,
    pub system_program: Program<System>,
}

fn main() {}
