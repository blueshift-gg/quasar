#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Instruction arg struct using the same String and Vec aliases as account
/// fields. Fixed composites normalize them to pod-backed fields internally.
#[derive(Copy, Clone, QuasarSerialize)]
pub struct CreateArgs {
    pub amount: u64,
    pub name: String<32>,
    pub tags: Vec<u8, 8>,
}

#[derive(Accounts)]
pub struct Create {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn create(
        _ctx: Ctx<Create>,
        _args: CreateArgs,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
