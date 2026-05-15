#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

#[derive(Accounts)]
#[instruction(amount: u64, amount: u8)]
pub struct Bad {
    pub signer: Signer,
}

fn main() {}
