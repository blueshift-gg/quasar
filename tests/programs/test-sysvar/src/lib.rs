#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

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

    #[instruction(discriminator = 3)]
    pub fn read_clock_full(ctx: Ctx<ReadClockFull>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn read_rent_calc(ctx: Ctx<ReadRentCalc>, data_len: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(data_len)
    }

    #[instruction(discriminator = 5)]
    pub fn read_clock_full_from_account(
        ctx: Ctx<ReadClockFullFromAccount>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
