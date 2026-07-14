#![allow(unexpected_cfgs)]
//! The `#[account(...)]` attribute grammar rejects unknown arguments with the
//! list of valid keys, instead of silently ignoring them.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1, mystery_flag)]
pub struct Bad {
    pub value: u64,
}

fn main() {}
