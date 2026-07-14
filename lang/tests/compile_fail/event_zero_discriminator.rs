#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// All-zero event discriminators are indistinguishable from zeroed event data,
// so they are rejected at any length.
#[event(discriminator = 0)]
pub struct ZeroEvent {
    pub value: u64,
}

fn main() {}
