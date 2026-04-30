#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
pub mod state;

declare_id!("11111111111111111111111111111113");

#[program]
mod quasar_test_one_of {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn check_consensus(ctx: Ctx<CheckConsensus>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn typed_accessor(ctx: Ctx<TypedAccessor>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
