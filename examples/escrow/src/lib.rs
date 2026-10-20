//! A maker deposits tokens of mint A and asks for an amount of mint B; a
//! taker pays it and receives the deposit, or the maker takes the deposit
//! back. Current Quasar's escrow example, written here, for Token and
//! Token-2022 alike.
#![no_std]

use quasar_lang::prelude::*;
use quasar_spl::prelude::*;

declare_id!("22222222222222222222222222222222222222222222");

#[account(discriminator = 1)]
#[seeds(b"escrow", maker: Address)]
pub struct Escrow {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}

#[event(discriminator = 0)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub deposit: u64,
    pub receive: u64,
}

#[event(discriminator = 1)]
pub struct TakeEvent {
    pub escrow: Address,
}

#[event(discriminator = 2)]
pub struct RefundEvent {
    pub escrow: Address,
}

/// What the maker deposits of mint A, and asks for of mint B.
#[derive(InstructionData)]
pub struct Offer {
    pub deposit: u64,
    pub receive: u64,
}

#[derive(Accounts)]
pub struct Make {
    #[account(mut)]
    pub maker: Signer,
    #[account(init(payer = maker), seeds(maker))]
    pub escrow: Pda<Account<Escrow>>,
    pub mint_a: InterfaceAccount<Mint>,
    pub mint_b: InterfaceAccount<Mint>,
    #[account(mut)]
    pub maker_ta_a: InterfaceAccount<TokenAccount>,
    #[account(init(payer = maker, idempotent), token(mint = mint_b, authority = maker))]
    pub maker_ta_b: InterfaceAccount<TokenAccount>,
    #[account(init(payer = maker, idempotent), token(mint = mint_a, authority = escrow))]
    pub vault_ta_a: InterfaceAccount<TokenAccount>,
    pub rent: Sysvar<Rent>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,
    #[account(
        mut,
        has_one(maker),
        has_one(maker_ta_b),
        check(escrow.receive() > 0),
        close(to = taker),
        seeds
    )]
    pub escrow: Pda<Account<Escrow>>,
    #[account(mut)]
    pub maker: UncheckedAccount,
    pub mint_a: InterfaceAccount<Mint>,
    pub mint_b: InterfaceAccount<Mint>,
    #[account(init(payer = taker, idempotent), token(mint = mint_a, authority = taker))]
    pub taker_ta_a: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub taker_ta_b: InterfaceAccount<TokenAccount>,
    #[account(init(payer = taker, idempotent), token(mint = mint_b, authority = maker))]
    pub maker_ta_b: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_ta_a: InterfaceAccount<TokenAccount>,
    pub rent: Sysvar<Rent>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Refund {
    #[account(mut)]
    pub maker: Signer,
    #[account(mut, has_one(maker), close(to = maker), seeds)]
    pub escrow: Pda<Account<Escrow>>,
    pub mint_a: InterfaceAccount<Mint>,
    #[account(init(payer = maker, idempotent), token(mint = mint_a, authority = maker))]
    pub maker_ta_a: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_ta_a: InterfaceAccount<TokenAccount>,
    pub rent: Sysvar<Rent>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}
#[program]
mod escrow {
    use super::*;

    #[instruction(0)]
    pub fn make(mut ctx: Ctx<Make, Offer>) -> Result<()> {
        let Offer { deposit, receive } = ctx.data;
        let Make {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ta_a,
            maker_ta_b,
            vault_ta_a,
            token_program,
            ..
        } = &mut ctx.accounts;
        escrow.set_inner(&Escrow {
            maker: *maker.address(),
            mint_a: *mint_a.address(),
            mint_b: *mint_b.address(),
            maker_ta_b: *maker_ta_b.address(),
            receive,
            bump: escrow.bump(),
        })?;
        emit!(MakeEvent {
            escrow: *escrow.address(),
            maker: *maker.address(),
            mint_a: *mint_a.address(),
            mint_b: *mint_b.address(),
            deposit,
            receive,
        });
        token::transfer(token_program, maker_ta_a, vault_ta_a, maker, deposit).invoke()
    }

    #[instruction(1)]
    pub fn take(ctx: Ctx<Take>) -> Result<()> {
        let Take {
            mut taker,
            escrow,
            mut taker_ta_a,
            mut taker_ta_b,
            mut maker_ta_b,
            mut vault_ta_a,
            token_program,
            ..
        } = ctx.accounts;
        let receive = escrow.receive();
        token::transfer(&token_program, &mut taker_ta_b, &mut maker_ta_b, &taker, receive)
            .invoke()?;
        let signer = escrow.signer();
        let amount = vault_ta_a.amount();
        token::transfer(&token_program, &mut vault_ta_a, &mut taker_ta_a, &escrow, amount)
            .invoke_signed(&signer)?;
        token::close_account(&token_program, vault_ta_a, &mut taker, &escrow)
            .invoke_signed(&signer)?;
        emit!(TakeEvent { escrow: *escrow.address() });
        Ok(())
    }

    #[instruction(2)]
    pub fn refund(ctx: Ctx<Refund>) -> Result<()> {
        let Refund { mut maker, escrow, mut maker_ta_a, mut vault_ta_a, token_program, .. } =
            ctx.accounts;
        let signer = escrow.signer();
        let amount = vault_ta_a.amount();
        token::transfer(&token_program, &mut vault_ta_a, &mut maker_ta_a, &escrow, amount)
            .invoke_signed(&signer)?;
        token::close_account(&token_program, vault_ta_a, &mut maker, &escrow)
            .invoke_signed(&signer)?;
        emit!(RefundEvent { escrow: *escrow.address() });
        Ok(())
    }
}
