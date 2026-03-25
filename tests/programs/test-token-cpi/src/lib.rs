#![no_std]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
declare_id!("88888888888888888888888888888888888888888888");

#[program]
mod quasar_test_token_cpi {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn transfer_checked(
        ctx: Ctx<TransferChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, decimals)
    }

    #[instruction(discriminator = 1)]
    pub fn approve(ctx: Ctx<Approve>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 2)]
    pub fn revoke(ctx: Ctx<Revoke>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 3)]
    pub fn mint_to(ctx: Ctx<MintTo>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 4)]
    pub fn burn(ctx: Ctx<Burn>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 5)]
    pub fn close_token_account(ctx: Ctx<CloseTokenAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 6)]
    pub fn interface_transfer(
        ctx: Ctx<InterfaceTransfer>,
        amount: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 7)]
    pub fn validate_ata_check(ctx: Ctx<ValidateAtaCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 8)]
    pub fn init_token_account(ctx: Ctx<InitTokenAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 9)]
    pub fn init_if_needed_token(ctx: Ctx<InitIfNeededToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 10)]
    pub fn init_ata(ctx: Ctx<InitAta>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 11)]
    pub fn init_if_needed_ata(ctx: Ctx<InitIfNeededAta>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 12)]
    pub fn init_mint_account(ctx: Ctx<InitMintAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 13)]
    pub fn init_mint_with_metadata(ctx: Ctx<InitMintWithMetadata>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 14)]
    pub fn init_if_needed_mint(ctx: Ctx<InitIfNeededMint>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 15)]
    pub fn init_if_needed_mint_with_freeze(
        ctx: Ctx<InitIfNeededMintWithFreeze>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 16)]
    pub fn validate_token_check(ctx: Ctx<ValidateTokenCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 17)]
    pub fn validate_token_interface_check(
        ctx: Ctx<ValidateTokenInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 18)]
    pub fn validate_ata_interface_check(
        ctx: Ctx<ValidateAtaInterfaceCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 19)]
    pub fn validate_token_no_program(
        ctx: Ctx<ValidateTokenNoProgram>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
