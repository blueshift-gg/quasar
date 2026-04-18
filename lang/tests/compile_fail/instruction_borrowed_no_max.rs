#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Args {
    pub signer: Signer,
}

#[program]
pub mod test_no_max {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn bad_handler(ctx: Ctx<Args>, label: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
