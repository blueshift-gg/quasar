#![allow(unexpected_cfgs)]
//! Proves that inherent `validate()` methods on Accounts structs are
//! called automatically before the instruction handler.
//!
//! No attribute is needed — the instruction macro always calls
//! `ctx.accounts.validate()`, and inherent methods shadow the trait default.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ── Account types ──

/// Struct with custom validation. Just write an inherent validate() method.
#[derive(Accounts)]
pub struct Guarded {
    pub signer: Signer,
}

impl Guarded {
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Custom pre-handler validation: e.g. check-not-paused, authority, etc.
        Ok(())
    }
}

/// Struct without validation — trait default (no-op) runs, LLVM elides it.
#[derive(Accounts)]
pub struct Unguarded {
    pub signer: Signer,
}

// ── Program ──

#[program]
pub mod test_validate {
    use super::*;

    // With validate
    #[instruction(discriminator = 0)]
    pub fn guarded_action(ctx: Ctx<Guarded>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    // Without validate — unaffected
    #[instruction(discriminator = 1)]
    pub fn open_action(ctx: Ctx<Unguarded>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

fn main() {}
