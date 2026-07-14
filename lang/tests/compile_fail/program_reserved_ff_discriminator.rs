#![allow(unexpected_cfgs)]
//! A `0xFF` (255) discriminator prefix is reserved for events and rejected for
//! instructions (program scan, reserved-value check).

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Initialize {
    pub signer: Signer,
}

#[program]
pub mod program_reserved_ff_discriminator {
    use super::*;

    #[instruction(discriminator = 255)]
    pub fn reserved(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

fn main() {}
