//! Token validation via op dispatch.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_spl::{*, TokenProgram};
use quasar_spl::ops::token;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct ValidateToken {
    pub authority: Signer,

    pub mint: Account<Mint>,

    #[account(
        token(mint = mint, authority = authority, token_program = token_program),
    )]
    pub vault: Account<Token>,

    pub token_program: Program<TokenProgram>,
}

fn main() {}
