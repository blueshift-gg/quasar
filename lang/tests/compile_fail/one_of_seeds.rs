#![allow(unexpected_cfgs)]
//! `#[seeds(...)]` on a `#[account(one_of)]` enum is a hard error, not a
//! silently-dropped attribute.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Settings {
    pub authority: Address,
}

#[account(one_of)]
#[seeds(b"consensus")]
pub enum ConsensusAccount {
    Settings(Settings),
}

fn main() {}
