#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program]
pub mod duplicate_instruction_macro_arg {
    use super::*;

    #[instruction(discriminator = 1, discriminator = 2)]
    pub fn bad(ctx: Context) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
