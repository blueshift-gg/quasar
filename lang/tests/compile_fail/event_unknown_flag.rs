#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// `#[event]` accepts only `discriminator`; `heap` (and any other flag) is
// rejected instead of being silently discarded.
#[event(heap)]
pub struct BadEvent {
    pub amount: u64,
}

fn main() {}
