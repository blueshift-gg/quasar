#![no_std]

use quasar_core::prelude::*;
use quasar_spl::{TokenProgram, TokenAccount};

declare_id!("22222222222222222222222222222222222222222222");

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: &'info mut Signer,
    #[account(seeds = [b"escrow", maker], bump)]
    pub escrow: &'info mut Initialize<EscrowAccount>,
    pub maker_ta_a: &'info mut Account<TokenAccount>,
    pub maker_ta_b: &'info Account<TokenAccount>,
    pub vault_ta_a: &'info mut Account<TokenAccount>,
    pub rent: &'info Rent,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

#[instruction(discriminator = 0)]
pub fn make(ctx: Ctx<Make>, deposit: u64, receive: u64) -> Result<(), ProgramError> {
    let seeds = ctx.bumps.escrow_seeds();

    EscrowAccount {
        maker: *ctx.accounts.maker.address(),
        mint_a: *ctx.accounts.maker_ta_a.mint(),
        mint_b: *ctx.accounts.maker_ta_b.mint(),
        maker_ta_b: *ctx.accounts.maker_ta_b.address(),
        receive,
        bump: ctx.bumps.escrow,
    }
    .init_signed(
        ctx.accounts.escrow,
        ctx.accounts.maker.to_account_view(),
        Some(ctx.accounts.rent),
        &[quasar_core::cpi::Signer::from(&seeds)],
    )?;

    ctx.accounts.token_program.transfer(
        ctx.accounts.maker_ta_a,
        ctx.accounts.vault_ta_a,
        ctx.accounts.maker,
        deposit,
    ).invoke()
}

#[account(discriminator = 1)]
pub struct EscrowAccount {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: &'info mut Signer,
    #[account(
        has_one = maker,
        constraint = escrow.receive > 0,
        seeds = [b"escrow", maker],
        bump = escrow.bump
    )]
    pub escrow: &'info Account<EscrowAccount>,
    pub maker: &'info UncheckedAccount,
    pub taker_ta_a: &'info mut Account<TokenAccount>,
    pub vault_ta_a: &'info mut Account<TokenAccount>,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

#[instruction(discriminator = 1)]
pub fn take(ctx: Ctx<Take>) -> Result<(), ProgramError> {
    let seeds = ctx.bumps.escrow_seeds();

    ctx.accounts.token_program.transfer(
        ctx.accounts.vault_ta_a,
        ctx.accounts.taker_ta_a,
        ctx.accounts.escrow,
        ctx.accounts.escrow.receive,
    ).invoke_signed(&seeds)?;

    ctx.accounts.token_program.close_account(
        ctx.accounts.vault_ta_a,
        ctx.accounts.taker,
        ctx.accounts.escrow,
    ).invoke_signed(&seeds)
}
