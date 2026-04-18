#![no_std]
#![allow(dead_code)]

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
    pub fn validate_token_no_program(ctx: Ctx<ValidateTokenNoProgram>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 20)]
    pub fn transfer_checked_t22(
        ctx: Ctx<TransferCheckedT22>,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, decimals)
    }

    #[instruction(discriminator = 21)]
    pub fn transfer_checked_interface(
        ctx: Ctx<TransferCheckedInterface>,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, decimals)
    }

    #[instruction(discriminator = 22)]
    pub fn approve_t22(ctx: Ctx<ApproveT22>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 23)]
    pub fn approve_interface(ctx: Ctx<ApproveInterface>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 24)]
    pub fn revoke_t22(ctx: Ctx<RevokeT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 25)]
    pub fn revoke_interface(ctx: Ctx<RevokeInterface>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 26)]
    pub fn mint_to_t22(ctx: Ctx<MintToT22>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 27)]
    pub fn mint_to_interface(ctx: Ctx<MintToInterface>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 28)]
    pub fn burn_t22(ctx: Ctx<BurnT22>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 29)]
    pub fn burn_interface(ctx: Ctx<BurnInterface>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 30)]
    pub fn close_token_account_t22(ctx: Ctx<CloseTokenAccountT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 31)]
    pub fn close_token_account_interface(
        ctx: Ctx<CloseTokenAccountInterface>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 32)]
    pub fn close_token(ctx: Ctx<CloseToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 33)]
    pub fn close_token_t22(ctx: Ctx<CloseTokenT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 34)]
    pub fn close_token_interface(ctx: Ctx<CloseTokenInterface>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 35)]
    pub fn sweep_token(ctx: Ctx<SweepToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 36)]
    pub fn sweep_and_close(ctx: Ctx<SweepAndClose>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 37)]
    pub fn sweep_token_t22(ctx: Ctx<SweepTokenT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 38)]
    pub fn sweep_token_interface(ctx: Ctx<SweepTokenInterface>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 39)]
    pub fn sweep_and_close_t22(ctx: Ctx<SweepAndCloseT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 40)]
    pub fn sweep_and_close_interface(ctx: Ctx<SweepAndCloseInterface>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
