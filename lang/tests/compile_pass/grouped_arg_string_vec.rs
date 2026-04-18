#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Grouped instruction arg struct with user-facing String and Vec fields.
/// Derives QuasarSerialize → InstructionArg and normalizes them to pod-backed
/// fixed fields internally.
#[derive(Copy, Clone, QuasarSerialize)]
pub struct MintArgs {
    pub amount: u64,
    pub name: String<32>,
    pub recipients: Vec<Address, 8>,
}

#[derive(Accounts)]
pub struct Mint {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn mint(
        _ctx: Ctx<Mint>,
        _args: MintArgs,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
