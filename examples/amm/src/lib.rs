//! A constant-product pool of two tokens. Depositors receive LP tokens, shares
//! of the reserves; swaps pay a fee into the reserves.
#![no_std]

pub mod math;

use quasar_lang::prelude::*;
use quasar_spl::prelude::*;

declare_id!("AMM1111111111111111111111111111111111111111");

#[account(discriminator = 1)]
#[seeds(b"pool", mint_a: Address, mint_b: Address)]
pub struct Pool {
    pub mint_a: Address,
    pub mint_b: Address,
    pub vault_a: Address,
    pub vault_b: Address,
    /// Its tokens are shares of the reserves.
    pub lp_mint: Address,
    pub fee_bps: u16,
    pub bump: u8,
}

#[error_code]
pub enum AmmError {
    #[msg("the fee is 100% or more")]
    Fee,
    #[msg("the result is worse than the limit given")]
    Slippage,
}

/// The pool's fee, in basis points of each swap's input.
#[derive(InstructionData)]
pub struct Fee {
    pub fee_bps: u16,
}

/// Pays at most `max_a` and `max_b` for at least `min_shares` LP tokens.
#[derive(InstructionData)]
pub struct Deposit {
    pub max_a: u64,
    pub max_b: u64,
    pub min_shares: u64,
}

/// Burns `shares` LP tokens for at least `min_a` and `min_b`.
#[derive(InstructionData)]
pub struct Withdraw {
    pub shares: u64,
    pub min_a: u64,
    pub min_b: u64,
}

/// Pays `amount_in` of mint A for mint B, or of mint B for mint A, for at
/// least `min_out`.
#[derive(InstructionData)]
pub struct Trade {
    pub amount_in: u64,
    pub min_out: u64,
    pub a_for_b: bool,
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    /// The same mint twice is a duplicate account, which loading rejects.
    pub mint_a: InterfaceAccount<Mint>,
    pub mint_b: InterfaceAccount<Mint>,
    #[account(init(payer = payer), seeds(mint_a, mint_b))]
    pub pool: Pda<Account<Pool>>,
    #[account(init(payer = payer), token(mint = mint_a, authority = pool))]
    pub vault_a: InterfaceAccount<TokenAccount>,
    #[account(init(payer = payer), token(mint = mint_b, authority = pool))]
    pub vault_b: InterfaceAccount<TokenAccount>,
    #[account(init(payer = payer), mint(decimals = 6, authority = pool))]
    pub lp_mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

/// Deposit and withdraw take the same accounts.
#[derive(Accounts)]
pub struct Liquidity {
    pub owner: Signer,
    #[account(has_one(vault_a), has_one(vault_b), has_one(lp_mint), seeds)]
    pub pool: Pda<Account<Pool>>,
    #[account(mut)]
    pub lp_mint: InterfaceAccount<Mint>,
    #[account(mut, token(authority = owner))]
    pub owner_a: InterfaceAccount<TokenAccount>,
    #[account(mut, token(authority = owner))]
    pub owner_b: InterfaceAccount<TokenAccount>,
    #[account(mut, token(mint = lp_mint, authority = owner))]
    pub owner_lp: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_a: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_b: InterfaceAccount<TokenAccount>,
    pub token_program: Interface<TokenInterface>,
}

#[derive(Accounts)]
pub struct Swap {
    pub owner: Signer,
    #[account(has_one(vault_a), has_one(vault_b), seeds)]
    pub pool: Pda<Account<Pool>>,
    #[account(mut, token(authority = owner))]
    pub owner_a: InterfaceAccount<TokenAccount>,
    #[account(mut, token(authority = owner))]
    pub owner_b: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_a: InterfaceAccount<TokenAccount>,
    #[account(mut)]
    pub vault_b: InterfaceAccount<TokenAccount>,
    pub token_program: Interface<TokenInterface>,
}
#[program]
mod amm {
    use super::*;

    #[instruction(0)]
    pub fn initialize(mut ctx: Ctx<Initialize, Fee>) -> Result<()> {
        let fee_bps = ctx.data.fee_bps;
        require!(u64::from(fee_bps) < math::FEE_DENOMINATOR, AmmError::Fee);
        let Initialize { mint_a, mint_b, pool, vault_a, vault_b, lp_mint, .. } = &mut ctx.accounts;
        pool.set_inner(&Pool {
            mint_a: *mint_a.address(),
            mint_b: *mint_b.address(),
            vault_a: *vault_a.address(),
            vault_b: *vault_b.address(),
            lp_mint: *lp_mint.address(),
            fee_bps,
            bump: pool.bump(),
        })?;
        Ok(())
    }

    #[instruction(1)]
    pub fn deposit(mut ctx: Ctx<Liquidity, Deposit>) -> Result<()> {
        let Deposit { max_a, max_b, min_shares } = ctx.data;
        let Liquidity {
            owner,
            pool,
            lp_mint,
            owner_a,
            owner_b,
            owner_lp,
            vault_a,
            vault_b,
            token_program,
        } = &mut ctx.accounts;
        let (shares, a, b) =
            math::deposit(max_a, max_b, vault_a.amount(), vault_b.amount(), lp_mint.supply())
                .ok_or(ProgramError::ArithmeticOverflow)?;
        require!(shares > 0 && shares >= min_shares, AmmError::Slippage);
        token::transfer(token_program, owner_a, vault_a, owner, a).invoke()?;
        token::transfer(token_program, owner_b, vault_b, owner, b).invoke()?;
        let signer = pool.signer();
        token::mint_to(token_program, lp_mint, owner_lp, pool, shares).invoke_signed(&signer)
    }

    #[instruction(2)]
    pub fn withdraw(mut ctx: Ctx<Liquidity, Withdraw>) -> Result<()> {
        let Withdraw { shares, min_a, min_b } = ctx.data;
        let Liquidity {
            owner,
            pool,
            lp_mint,
            owner_a,
            owner_b,
            owner_lp,
            vault_a,
            vault_b,
            token_program,
        } = &mut ctx.accounts;
        let (a, b) = math::withdraw(shares, vault_a.amount(), vault_b.amount(), lp_mint.supply())
            .ok_or(ProgramError::ArithmeticOverflow)?;
        require!(a >= min_a && b >= min_b, AmmError::Slippage);
        token::burn(token_program, owner_lp, lp_mint, owner, shares).invoke()?;
        let signer = pool.signer();
        token::transfer(token_program, vault_a, owner_a, pool, a).invoke_signed(&signer)?;
        token::transfer(token_program, vault_b, owner_b, pool, b).invoke_signed(&signer)
    }

    #[instruction(3)]
    pub fn swap(mut ctx: Ctx<Swap, Trade>) -> Result<()> {
        let Trade { amount_in, min_out, a_for_b } = ctx.data;
        let Swap { owner, pool, owner_a, owner_b, vault_a, vault_b, token_program } =
            &mut ctx.accounts;
        let (owner_in, owner_out, vault_in, vault_out) = if a_for_b {
            (owner_a, owner_b, vault_a, vault_b)
        } else {
            (owner_b, owner_a, vault_b, vault_a)
        };
        let fee = u64::from(pool.fee_bps());
        let out = math::swap(amount_in, vault_in.amount(), vault_out.amount(), fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        require!(out > 0 && out >= min_out, AmmError::Slippage);
        token::transfer(token_program, owner_in, vault_in, owner, amount_in).invoke()?;
        let signer = pool.signer();
        token::transfer(token_program, vault_out, owner_out, pool, out).invoke_signed(&signer)
    }
}
