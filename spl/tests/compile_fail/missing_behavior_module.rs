#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 99)]
pub struct MyData {
    pub value: u64,
}

// ERROR: no module `nonexistent` in scope: produces a readable Rust error
#[derive(Accounts)]
pub struct Bad {
    #[account(nonexistent(value = 42u64))]
    pub data: Account<MyData>,
}

fn main() {}
