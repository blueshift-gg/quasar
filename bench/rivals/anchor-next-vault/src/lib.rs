//! Current Quasar's vault written with anchor-next v2, the way its own bench
//! programs are written: the vault is a system-owned PDA, and withdrawing is
//! a System transfer the PDA signs.

use anchor_lang::prelude::*;
use pinocchio::cpi::{Seed, Signer as SignerSeeds};

declare_id!("33333333333333333333333333333333333333333333");

#[program]
pub mod anchor_next_vault {
    use super::*;

    #[discrim = 0]
    pub fn deposit(ctx: &mut Context<Deposit>, amount: u64) -> Result<()> {
        pinocchio_system::instructions::Transfer {
            from: ctx.accounts.user.account(),
            to: ctx.accounts.vault.account(),
            lamports: amount,
        }
        .invoke()?;
        Ok(())
    }

    #[discrim = 1]
    pub fn withdraw(ctx: &mut Context<Withdraw>, amount: u64) -> Result<()> {
        let bump = [ctx.bumps.vault];
        let user = ctx.accounts.user.address();
        let seeds = [Seed::from(b"vault"), Seed::from(user.as_ref()), Seed::from(&bump)];
        pinocchio_system::instructions::Transfer {
            from: ctx.accounts.vault.account(),
            to: ctx.accounts.user.account(),
            lamports: amount,
        }
        .invoke_signed(&[SignerSeeds::from(&seeds)])?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [b"vault", user.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [b"vault", user.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}
