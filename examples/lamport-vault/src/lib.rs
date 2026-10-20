//! A vault this program owns: deposits arrive through the System Program, and
//! withdrawals take lamports straight out, with no call. Blueshift Quasar's
//! vault example, written here.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("33333333333333333333333333333333333333333333");

#[derive(Seeds)]
#[seeds(b"vault", user: Address)]
pub struct VaultPda;

/// Lamports to move.
#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds)]
    pub vault: Pda<UncheckedAccount, VaultPda>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds)]
    pub vault: Pda<UncheckedAccount, VaultPda>,
}
#[program]
mod lamport_vault {
    use super::*;

    #[instruction(0)]
    pub fn deposit(mut ctx: Ctx<Deposit, Amount>) -> Result<()> {
        let Deposit { user, vault, system_program } = &mut ctx.accounts;
        system_program.transfer(user, vault, ctx.data.amount).invoke()
    }

    #[instruction(1)]
    pub fn withdraw(mut ctx: Ctx<Withdraw, Amount>) -> Result<()> {
        let Withdraw { user, vault } = &mut ctx.accounts;
        vault.sub_lamports(ctx.data.amount)?;
        user.add_lamports(ctx.data.amount)
    }
}
