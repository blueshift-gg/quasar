//! Init with PDA signer seeds.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 5)]
#[seeds(b"escrow", authority: Address)]
pub struct Escrow {
    pub authority: Address,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitEscrow {
    #[account(mut)]
    pub payer: Signer,

    #[account(mut,
        init, payer = payer,
        address = Escrow::seeds(payer.address()),
    )]
    pub escrow: Account<Escrow>,

    pub system_program: Program<SystemProgram>,
}

fn main() {}
