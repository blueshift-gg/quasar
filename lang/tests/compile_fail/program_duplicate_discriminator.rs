#![allow(unexpected_cfgs)]
//! Two instructions pinned to the same discriminator is a hard error that names
//! the prior owner (program scan, first pass).

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Initialize {
    pub signer: Signer,
}

#[program]
pub mod program_duplicate_discriminator {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn foo(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 1)]
    pub fn bar(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

fn main() {}
