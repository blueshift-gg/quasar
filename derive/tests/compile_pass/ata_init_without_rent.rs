//! ATA init does not require Quasar to materialize Rent.
#![allow(unexpected_cfgs)]

extern crate alloc;

use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{accounts::associated_token, TokenProgram, *},
};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct InitAtaWithoutRent {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    #[account(mut,
        init(idempotent),
        associated_token(
            authority = payer, mint = mint,
            token_program = token_program,
            system_program = system_program,
            ata_program = ata_program,
        ),
    )]
    pub ata_vault: Account<Token>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}

fn main() {}
