#![no_std]

use quasar_lang::prelude::*;

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
    pub fn init_max_seed_length(ctx: Ctx<InitMaxSeedLength>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = 9)]
    pub fn init_three_seeds(ctx: Ctx<InitThreeSeeds>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }

    #[instruction(discriminator = 10)]
    pub fn init_max_multi_seeds(ctx: Ctx<InitMaxMultiSeeds>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 11)]
    pub fn init_ix_data_seed(ctx: Ctx<InitIxDataSeed>, index: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(index, &ctx.bumps)
    }

    #[instruction(discriminator = 12)]
    pub fn init_ns_config(ctx: Ctx<InitNsConfig>, namespace: u32) -> Result<(), ProgramError> {
        ctx.accounts.handler(namespace, &ctx.bumps)
    }

    #[instruction(discriminator = 13)]
    pub fn init_scoped_item(ctx: Ctx<InitScopedItem>, namespace: u32) -> Result<(), ProgramError> {
        ctx.accounts.handler(namespace, &ctx.bumps)
    }

    #[instruction(discriminator = 14)]
    pub fn verify_scoped_item(ctx: Ctx<VerifyScopedItem>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 15)]
    pub fn init_const_seed(ctx: Ctx<InitConstSeed>) -> Result<(), ProgramError> {
        ctx.accounts.handler(&ctx.bumps)
    }
}
