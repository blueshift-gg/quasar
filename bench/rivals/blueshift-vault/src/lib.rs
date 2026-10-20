//! Current Quasar's vault written with Blueshift's Quasar, following its own
//! vault example, except that withdrawing is a System transfer the PDA signs,
//! as in every other implementation here.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("33333333333333333333333333333333333333333333");

#[derive(Seeds)]
#[seeds(b"vault", user: Address)]
pub struct VaultPda;

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, address = VaultPda::seeds(user.address()))]
    pub vault: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, address = VaultPda::seeds(user.address()))]
    pub vault: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

#[program]
mod blueshift_vault {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn deposit(ctx: Ctx<Deposit>, amount: u64) -> Result<(), ProgramError> {
        let accounts = &ctx.accounts;
        accounts.system_program.transfer(&accounts.user, &accounts.vault, amount).invoke()
    }

    #[instruction(discriminator = 1)]
    pub fn withdraw(ctx: Ctx<Withdraw>, amount: u64) -> Result<(), ProgramError> {
        let accounts = &ctx.accounts;
        let signer = accounts.vault_signer(&ctx.bumps);
        accounts.system_program.transfer(&accounts.vault, &accounts.user, amount).invoke_signed(&signer)
    }
}
