#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;
use quasar_spl::{Mint, Token, Token2022};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BadTokenProgramSelector {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    pub mint: Account<Mint>,
    #[account(
        mut,
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
    )]
    pub token: Account<Token>,
    pub token_program: Program<Token>,
    pub token_program_2022: Program<Token2022>,
    pub system_program: Program<System>,
}

fn main() {}
