#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Composite struct containing user-facing String and Vec aliases.
/// QuasarSerialize normalizes them to pod-backed fixed fields internally.
#[derive(Copy, Clone, QuasarSerialize)]
pub struct Metadata {
    pub label: String<16>,
    pub values: Vec<u8, 4>,
    pub version: u32,
}

/// Account with composite field containing aliased String/Vec.
/// Tests the full chain: alias rewriting → ZC mapping → set_inner codegen.
#[account(discriminator = 1, set_inner)]
pub struct Registry {
    pub meta: Metadata,
    pub bump: u8,
}

/// Instruction arg with composite containing aliased String/Vec.
#[derive(Copy, Clone, QuasarSerialize)]
pub struct UpdateArgs {
    pub meta: Metadata,
    pub flag: bool,
}

#[derive(Accounts)]
pub struct Update {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn update(
        _ctx: Ctx<Update>,
        _args: UpdateArgs,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
