#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program]
pub mod test_raw_extra {
    use super::*;

    #[instruction(discriminator = 1, raw)]
    pub fn bad_handler(ctx: Context, value: u64) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
