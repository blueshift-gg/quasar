#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

const MAX_SIGNERS: usize = 10;

#[account(discriminator = 1, set_inner)]
pub struct Vault {
    pub authority: Address,
    pub signers: Vec<Address, MAX_SIGNERS>,
}

fn main() {}
