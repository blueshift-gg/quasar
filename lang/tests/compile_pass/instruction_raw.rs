#![allow(unexpected_cfgs)]
//! Proves that `#[instruction(raw)]` compiles: a program with both normal
//! and raw instructions coexisting in the same module.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Initialize {
    pub signer: Signer,
}

#[program]
pub mod test_raw {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 1, raw)]
    pub fn update_oracle(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.is_empty() {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let _ = ctx.data;
        Ok(())
    }
}

fn main() {}
