#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
declare_id!("22222222222222222222222222222222222222222222");

#[program]
mod quasar_test_token_validate {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn validate_token_check(ctx: Ctx<ValidateTokenCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn validate_token_2022_check(ctx: Ctx<ValidateToken2022Check>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn validate_token_interface_check(
        ctx: Ctx<ValidateTokenInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 3)]
    pub fn validate_mint_check(ctx: Ctx<ValidateMintCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn validate_mint_2022_check(ctx: Ctx<ValidateMint2022Check>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 5)]
    pub fn validate_mint_interface_check(
        ctx: Ctx<ValidateMintInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 6)]
    pub fn validate_ata_check(ctx: Ctx<ValidateAtaCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 7)]
    pub fn validate_ata_2022_check(ctx: Ctx<ValidateAta2022Check>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 8)]
    pub fn validate_ata_interface_check(
        ctx: Ctx<ValidateAtaInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 9)]
    pub fn validate_token_no_program(ctx: Ctx<ValidateTokenNoProgram>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 10)]
    pub fn validate_mint_no_program(ctx: Ctx<ValidateMintNoProgram>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 11)]
    pub fn validate_mint_with_freeze_check(
        ctx: Ctx<ValidateMintWithFreezeCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 12)]
    pub fn validate_mint_with_freeze_2022_check(
        ctx: Ctx<ValidateMintWithFreeze2022Check>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 13)]
    pub fn validate_mint_with_freeze_interface_check(
        ctx: Ctx<ValidateMintWithFreezeInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
