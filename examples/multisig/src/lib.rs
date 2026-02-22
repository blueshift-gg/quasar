#![no_std]

#[cfg(feature = "client")]
extern crate alloc;
#[cfg(feature = "client")]
pub mod client;
use quasar_core::prelude::*;

mod instructions;
use instructions::*;
mod state;
#[cfg(test)]
mod tests;

declare_id!("44444444444444444444444444444444444444444444");

#[program]
mod quasar_multisig {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn create(ctx: Ctx<Create>, threshold: u8) -> Result<(), ProgramError> {
        ctx.accounts.create_multisig(threshold, &ctx.bumps, ctx.remaining_accounts())
    }

    #[instruction(discriminator = 1)]
    pub fn deposit(ctx: Ctx<Deposit>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.deposit(amount)
    }

    #[instruction(discriminator = 2)]
    pub fn set_label(ctx: Ctx<SetLabel>, label_len: u8, label_bytes: [u8; 32]) -> Result<(), ProgramError> {
        ctx.accounts.update_label(label_len, &label_bytes)
    }

    #[instruction(discriminator = 3)]
    pub fn execute_transfer(ctx: Ctx<ExecuteTransfer>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.verify_and_transfer(amount, &ctx.bumps, ctx.remaining_accounts())
    }
}
