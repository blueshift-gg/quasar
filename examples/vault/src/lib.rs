#![no_std]

use quasar_lang::prelude::*;

mod instructions;
pub use instructions::*;

declare_id!("33333333333333333333333333333333333333333333");
#[program]
mod quasar_vault {
    use super::*;

    #[instruction(0)]
    pub fn deposit(mut ctx: Ctx<Deposit, Amount>) -> Result<(), ProgramError> {
        ctx.accounts.deposit(ctx.data.amount)
    }

    #[instruction(1)]
    pub fn withdraw(mut ctx: Ctx<Withdraw, Amount>) -> Result<(), ProgramError> {
        ctx.accounts.withdraw(ctx.data.amount)
    }
}
