#![no_std]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
declare_id!("1nit111111111111111111111111111111111111111");

#[program]
mod quasar_test_token_init {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn init_token(ctx: Ctx<InitToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn init_if_needed_token(ctx: Ctx<InitIfNeededToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn init_ata(ctx: Ctx<InitAta>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 3)]
    pub fn init_if_needed_ata(ctx: Ctx<InitIfNeededAta>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn init_mint(ctx: Ctx<InitMint>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 5)]
    pub fn init_if_needed_mint(ctx: Ctx<InitIfNeededMint>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 6)]
    pub fn init_if_needed_mint_with_freeze(ctx: Ctx<InitIfNeededMintWithFreeze>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 7)]
    pub fn init_mint_with_metadata(ctx: Ctx<InitMintWithMetadata>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 8)]
    pub fn init_token_t22(ctx: Ctx<InitTokenT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 9)]
    pub fn init_if_needed_token_t22(ctx: Ctx<InitIfNeededTokenT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 10)]
    pub fn init_ata_t22(ctx: Ctx<InitAtaT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 11)]
    pub fn init_if_needed_ata_t22(ctx: Ctx<InitIfNeededAtaT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 12)]
    pub fn init_mint_t22(ctx: Ctx<InitMintT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 13)]
    pub fn init_if_needed_mint_t22(ctx: Ctx<InitIfNeededMintT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 14)]
    pub fn init_if_needed_mint_with_freeze_t22(ctx: Ctx<InitIfNeededMintWithFreezeT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
