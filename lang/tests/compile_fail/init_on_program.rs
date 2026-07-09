#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ERROR: `init` requires a program data account (`Account<T>` /
// `InterfaceAccount<T>`), not a `Program`. The type gate must reject this at
// the field with one spanned error, not an `E0277` dump quoting `AccountInit`.
#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer)]
    pub target: Program<SystemProgram>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
