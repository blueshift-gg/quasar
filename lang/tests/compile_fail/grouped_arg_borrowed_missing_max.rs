#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(QuasarSerialize)]
pub struct BadArgs<'a> {
    pub amount: u64,
    pub name: &'a str,
}

fn main() {}
