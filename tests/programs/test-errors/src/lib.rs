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

    #[instruction(discriminator = 12)]
    pub fn header_nodup_mut_signer(ctx: Ctx<HeaderNoDupMutSigner>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 13)]
    pub fn header_nodup_mut(ctx: Ctx<HeaderNoDupMut>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 14)]
    pub fn header_nodup_signer(ctx: Ctx<HeaderNoDupSigner>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 15)]
    pub fn header_executable(ctx: Ctx<HeaderExecutable>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 16)]
    pub fn header_dup_mut(ctx: Ctx<HeaderDupMut>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 17)]
    pub fn header_dup_signer(ctx: Ctx<HeaderDupSigner>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 20)]
    pub fn system_account_check(ctx: Ctx<SystemAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 21)]
    pub fn program_check(ctx: Ctx<ProgramCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 22)]
    pub fn signer_mut_check(ctx: Ctx<SignerMutCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 23)]
    pub fn unchecked_account_check(ctx: Ctx<UncheckedAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 24)]
    pub fn two_accounts_check(ctx: Ctx<TwoAccountsCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 25)]
    pub fn signer_readonly_check(ctx: Ctx<SignerReadonlyCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 26)]
    pub fn three_accounts_dup(ctx: Ctx<ThreeAccountsDup>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 27)]
    pub fn has_one_default(ctx: Ctx<HasOneDefault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 28)]
    pub fn address_default(ctx: Ctx<AddressDefault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 29)]
    pub fn constraint_default(ctx: Ctx<ConstraintDefault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
