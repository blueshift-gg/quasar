use quasar_core::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct BadDynamic<'a> {
    pub vals: Vec<'a, u64, 2>,
}

fn main() {}
