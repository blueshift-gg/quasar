//! PDA account with typed seeds and bump.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_lang::prelude::*;
use quasar_derive::Accounts;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 2)]
#[seeds(b"escrow", authority: Address)]
pub struct Escrow {
    pub authority: Address,
    pub amount: u64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct ValidateEscrow {
    pub authority: Signer,

    #[account(
        address = Escrow::seeds(authority.address()),
    )]
    pub escrow: Account<Escrow>,
}

fn main() {}
