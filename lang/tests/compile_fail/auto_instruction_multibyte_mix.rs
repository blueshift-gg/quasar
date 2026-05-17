#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Initialize {
    pub signer: Signer,
}

#[program]
pub mod auto_instruction_multibyte_mix {
    use super::*;

    #[instruction]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = [1, 2])]
    pub fn pinned(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

fn main() {}
