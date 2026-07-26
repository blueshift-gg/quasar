//! An `Address` instruction argument used as a PDA seed.
//!
//! The seed set owns its seeds, so the generated signer helper can return one
//! built from an instruction argument, which is a local.
#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_derive::Accounts;
use quasar_lang::prelude::*;
solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"v", authority: Address)]
pub struct V {
    pub authority: Address,
}

#[derive(Accounts)]
#[instruction(authority: Address)]
pub struct ReadIt {
    #[account(address = V::seeds(&authority))]
    pub v: Account<V>,
}

#[derive(Accounts)]
#[instruction(authority: Address, id: u64)]
pub struct InitIt {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, address = V::seeds(&authority))]
    pub v: Account<V>,
    pub system_program: Program<SystemProgram>,
}
fn main() {}
