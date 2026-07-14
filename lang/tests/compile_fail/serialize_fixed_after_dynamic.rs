#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// A fixed field after a borrowed (compact tail) field is rejected: the compact
// wire layout requires all fixed fields to precede the dynamic tail.
#[derive(QuasarSerialize)]
pub struct BadOrder<'a> {
    #[max(8)]
    pub name: &'a str,
    pub amount: u64,
}

fn main() {}
