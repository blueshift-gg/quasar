#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Args {
    pub signer: Signer,
}

#[program]
pub mod test_raw_wrong {
    use super::*;

    #[instruction(discriminator = 1, raw)]
    pub fn bad_handler(ctx: Ctx<Args>) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
