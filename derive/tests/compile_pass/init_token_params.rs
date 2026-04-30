//! Init with token params inferred from sibling token(...) group.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_spl::{*, TokenProgram};
use quasar_spl::ops::token;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 10)]
#[seeds(b"vault", authority: Address)]
pub struct Vault {
    pub authority: Address,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitTokenVault {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    #[account(mut,
        init, payer = payer,
        address = Vault::seeds(payer.address()),
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub vault: Account<Token>,

    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
