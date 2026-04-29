#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program]
pub mod test_raw_hook {
    use super::*;

    #[instruction(discriminator = 1, raw, pre_hook = [my_hook])]
    pub fn bad_handler(ctx: Context) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
