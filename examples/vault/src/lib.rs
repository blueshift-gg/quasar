#![no_std]

#[cfg(feature = "client")]
extern crate alloc;
#[cfg(feature = "client")]
pub mod client;
use quasar_core::prelude::*;

panic_handler!();
no_alloc!();

mod instructions;
use instructions::*;
#[cfg(test)]
mod tests;

declare_id!("33333333333333333333333333333333333333333333");

#[program]
mod quasar_vault {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn deposit(ctx: Ctx<Deposit>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.deposit(amount)
    }

    #[instruction(discriminator = 1)]
    pub fn withdraw(ctx: Ctx<Withdraw>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.withdraw(amount, &ctx.bumps)
    }
}
