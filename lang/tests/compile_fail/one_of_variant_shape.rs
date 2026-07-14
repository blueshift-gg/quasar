#![allow(unexpected_cfgs)]
//! Each `#[account(one_of)]` variant must wrap exactly one unnamed account type
//! (`Variant(Type)`). A named-field variant is a hard error.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Settings {
    pub authority: Address,
}

#[account(discriminator = 2)]
pub struct Config {
    pub authority: Address,
}

#[account(one_of)]
pub enum ConsensusAccount {
    Named { settings: Settings },
    Other(Config),
}

fn main() {}
