#![allow(unexpected_cfgs)]
//! A module-qualified accounts type in `Ctx<instructions::Deposit>` must
//! resolve the IDL/client fragment name from the last path segment
//! (`Deposit`) instead of panicking in `format_ident!` on the `::`-bearing
//! string, and must still join to the accounts-meta fragment (bare `Deposit`).

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

pub mod instructions {
    use super::*;

    #[derive(Accounts)]
    pub struct Deposit {
        pub signer: Signer,
    }
}

#[program(no_entrypoint)]
pub mod test_ctx_path {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn deposit(ctx: Ctx<instructions::Deposit>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

fn main() {}
