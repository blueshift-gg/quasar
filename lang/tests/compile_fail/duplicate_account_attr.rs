#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    #[account(mut)]
    pub signer: Signer,
}

fn main() {}
