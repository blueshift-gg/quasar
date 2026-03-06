use quasar_core::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct BadOrder<'a> {
    pub name: String<u32, 32>,
    pub value: u64,
}

fn main() {}
