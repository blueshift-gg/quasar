#![allow(unexpected_cfgs)]
//! Proves that `#[instruction(raw, heap)]` compiles and composes with
//! a normal non-heap instruction in the same program.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Simple {
    pub signer: Signer,
}

#[program]
pub mod test_raw_heap {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn normal(ctx: Ctx<Simple>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 1, raw, heap)]
    pub fn batch_update(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.accounts.len();
        Ok(())
    }
}

fn main() {}
