#![no_std]

use quasar_core::prelude::*;

mod instructions;
use instructions::*;
pub mod state;
declare_id!("77777777777777777777777777777777777777777777");

#[program]
mod quasar_test_pda {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn init_literal_seed(ctx: Ctx<InitLiteralSeed>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = 1)]
    pub fn init_pubkey_seed(ctx: Ctx<InitPubkeySeed>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 2)]
    pub fn init_instruction_seed(
        ctx: Ctx<InitInstructionSeed>,
        id: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(id, &ctx.bumps)
    }

    #[instruction(discriminator = 3)]
    pub fn init_multi_seeds(ctx: Ctx<InitMultiSeeds>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, &ctx.bumps)
    }

    #[instruction(discriminator = 4)]
    pub fn update_pda(ctx: Ctx<UpdatePda>, new_value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_value)
    }

    #[instruction(discriminator = 5)]
    pub fn close_pda(ctx: Ctx<ClosePda>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 6)]
    pub fn pda_transfer(ctx: Ctx<PdaTransfer>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 7)]
    pub fn init_empty_seed(ctx: Ctx<InitEmptySeed>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = 8)]
    pub fn init_max_seed(ctx: Ctx<InitMaxSeed>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = 9)]
    pub fn init_three_seeds(ctx: Ctx<InitThreeSeeds>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }
}
