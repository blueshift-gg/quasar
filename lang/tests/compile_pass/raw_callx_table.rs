#![allow(unexpected_cfgs)]
//! Proves that contiguous 1-byte raw discriminators compile to the O(1)
//! function pointer table dispatch path (callx). The discriminators 0, 1, 2
//! are contiguous, so the derive emits an indexed table instead of a match chain.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program]
pub mod test_raw_callx {
    use super::*;

    /// Three contiguous raw instructions → function pointer table dispatch.
    #[instruction(discriminator = 0, raw)]
    pub fn fast_a(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }

    #[instruction(discriminator = 1, raw)]
    pub fn fast_b(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }

    #[instruction(discriminator = 2, raw)]
    pub fn fast_c(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }
}

fn main() {}
