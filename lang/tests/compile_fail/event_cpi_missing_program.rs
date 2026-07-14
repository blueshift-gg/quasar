#![allow(unexpected_cfgs)]
//! An accounts struct with an `event_authority` field but no `Program<...>`
//! field cannot service event CPI: the derive rejects it with a spanned error
//! (previously it silently generated no `EventCpi` impl).

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program]
pub mod event_cpi_missing_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn go(ctx: Ctx<MissingProgram>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct MissingProgram {
    pub signer: Signer,
    pub event_authority: EventAuthority,
}

fn main() {}
