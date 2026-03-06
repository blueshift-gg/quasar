use quasar_core::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = [1])]
pub struct BadMultiTail<'a> {
    pub label: &'a str,
    pub data: &'a [u8],
}

fn main() {}
