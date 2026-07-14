#![allow(unexpected_cfgs)]
//! `address = seeds(..) @ error` must keep the field in the generated `Bumps`
//! struct (and its stored-bump fast path, `{field}_signer` helper, and IDL PDA
//! resolver). Before the fix the `@ error` form was rerouted into `user_checks`
//! and the field silently dropped out of `Bumps`.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[error_code]
pub enum MyError {
    WrongEscrow,
}

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

    #[account(address = Escrow::seeds(authority.address()) @ MyError::WrongEscrow)]
    pub escrow: Account<Escrow>,
}

// If the field were dropped from `Bumps` (the bug), `bumps.escrow` -- and the
// `escrow_signer` helper, which is only generated for a seeds-based address --
// would not exist and this would fail to compile.
fn _bumps_retains_escrow(bumps: &ValidateEscrowBumps) -> u8 {
    bumps.escrow
}

fn _signer_helper_exists(accounts: &ValidateEscrow, bumps: &ValidateEscrowBumps) {
    let _ = accounts.escrow_signer(bumps);
}

fn main() {}
