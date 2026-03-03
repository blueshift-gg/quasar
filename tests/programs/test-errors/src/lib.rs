#![no_std]
#![allow(dead_code)]

use quasar_core::prelude::*;

mod instructions;
use instructions::*;
pub mod errors;
pub mod state;
declare_id!("55555555555555555555555555555555555555555555");

#[program]
mod quasar_test_errors {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn custom_error(ctx: Ctx<CustomError>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn explicit_error(ctx: Ctx<ExplicitError>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn require_false(ctx: Ctx<RequireFalse>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 3)]
    pub fn program_error(ctx: Ctx<ProgramErrorIx>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn require_eq_check(ctx: Ctx<RequireEqCheck>, a: u64, b: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b)
    }

    #[instruction(discriminator = 5)]
    pub fn require_neq_check(
        ctx: Ctx<RequireNeqCheck>,
        a: u64,
        b: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b)
    }

    #[instruction(discriminator = 6)]
    pub fn constraint_fail(ctx: Ctx<ConstraintFail>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 7)]
    pub fn has_one_custom(ctx: Ctx<HasOneCustom>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 8)]
    pub fn signer_needed(ctx: Ctx<SignerNeeded>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 9)]
    pub fn account_check(ctx: Ctx<AccountCheckIx>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 10)]
    pub fn mut_account_check(ctx: Ctx<MutAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 11)]
    pub fn address_custom_error(ctx: Ctx<AddressCustomError>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
