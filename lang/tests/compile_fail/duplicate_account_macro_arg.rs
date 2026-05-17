use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1, discriminator = 2)]
pub struct Bad {}

fn main() {}
