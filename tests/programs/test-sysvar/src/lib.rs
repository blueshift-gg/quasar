#![no_std]

use quasar_core::prelude::*;

mod instructions;
use instructions::*;
pub mod state;
declare_id!("99999999999999999999999999999999999999999999");

#[program]
mod quasar_test_sysvar {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn read_clock(ctx: Ctx<ReadClock>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn read_rent(ctx: Ctx<ReadRent>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn read_clock_from_account(ctx: Ctx<ReadClockFromAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
