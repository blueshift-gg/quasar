//! Fully-qualified namespaced behavior path without a local `use` import.
#![allow(unexpected_cfgs)]
extern crate alloc;
use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token, TokenProgram},
};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct NamespacedBehavior {
    pub authority: Signer,
    pub mint: Account<Mint>,

    #[account(
        quasar_spl::accounts::token(
            mint = mint,
            authority = authority,
            token_program = token_program,
        ),
    )]
    pub vault: Account<Token>,

    pub token_program: Program<TokenProgram>,
}

fn main() {}
