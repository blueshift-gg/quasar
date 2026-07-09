#![allow(unexpected_cfgs)]
//! An invalid explicit length-prefix width (`f32`) on a dynamic account field
//! is a hard error, not a silent fall-back to a 1-byte prefix.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct BadPrefix {
    pub name: String<16, f32>,
}

fn main() {}
