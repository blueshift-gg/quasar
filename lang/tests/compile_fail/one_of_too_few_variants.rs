#![allow(unexpected_cfgs)]
//! A `#[account(one_of)]` enum must declare at least two variants; one variant
//! is a hard error, not a silently-accepted degenerate polymorphic account.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Settings {
    pub authority: Address,
}

#[account(one_of)]
pub enum ConsensusAccount {
    Settings(Settings),
}

fn main() {}
