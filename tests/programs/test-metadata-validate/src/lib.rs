#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
declare_id!("11111111111111111111111111111113");

#[program]
mod quasar_test_metadata_validate {
    use super::*;

    /// Metadata with behavior: PDA + mint cross-validation.
    #[instruction(discriminator = 0)]
    pub fn validate_metadata_check(ctx: Ctx<ValidateMetadataCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Metadata with behavior + update_authority check.
    #[instruction(discriminator = 1)]
    pub fn validate_metadata_with_ua(ctx: Ctx<ValidateMetadataWithUa>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Master edition with behavior: PDA check.
    #[instruction(discriminator = 2)]
    pub fn validate_master_edition_check(
        ctx: Ctx<ValidateMasterEditionCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Bare metadata only runs AccountLoad checks (owner + key byte +
    /// data_len).
    #[instruction(discriminator = 3)]
    pub fn validate_bare_metadata(ctx: Ctx<ValidateBareMetadata>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Bare master edition only runs AccountLoad checks.
    #[instruction(discriminator = 4)]
    pub fn validate_bare_master_edition(
        ctx: Ctx<ValidateBareMasterEdition>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Init metadata via CPI + verify all prefix fields.
    #[instruction(discriminator = 10)]
    pub fn init_metadata_test(ctx: Ctx<InitMetadataTest>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Init master edition via CPI + verify all prefix fields.
    #[instruction(discriminator = 11)]
    pub fn init_master_edition_test(ctx: Ctx<InitMasterEditionTest>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
