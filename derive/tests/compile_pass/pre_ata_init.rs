//! ATA init — clean syntax with init + ata_init.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_spl::{*, TokenProgram};
use quasar_spl::ops::ata_init;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct InitAta {
    #[account(mut)]
    pub payer: Signer,

    pub mint: Account<Mint>,

    #[account(mut,
        init, payer = payer,
        ata_init(
            authority = payer, mint = mint, payer = payer, token_program = token_program,
            system_program = system_program, ata_program = ata_program,
            idempotent = false,
        ),
    )]
    pub ata_vault: Account<Token>,

    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}

fn main() {}
