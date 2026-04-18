#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;
use quasar_spl::{Mint, Token, Token2022};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BadMintProgramSelector {
    #[account(mut)]
    pub payer: Signer,
    pub mint_authority: Signer,
    #[account(
        mut,
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = mint_authority,
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub token_program_2022: Program<Token2022>,
    pub system_program: Program<System>,
}

fn main() {}
