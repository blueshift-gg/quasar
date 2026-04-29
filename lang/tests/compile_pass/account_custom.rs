#![allow(unexpected_cfgs)]
//! Proves that `#[account(custom)]` works in two modes:
//!
//! 1. **Unit struct** — transparent AccountView wrapper, user provides check().
//!    Use case: accept any account, verify later (ResolvedSigner).
//!
//! 2. **Struct with fields** — full zero-copy typed access, user provides check()
//!    instead of framework owner/discriminator checks.
//!    Use case: typed data with custom validation logic.
//!
//! For full manual control, users can implement `#[repr(transparent)]` +
//! `AsAccountView` + `AccountLoad` directly without `#[account(custom)]`.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ── Mode 1: unit struct (no data, just a wrapper) ──

#[account(custom)]
pub struct ResolvedSigner;

impl ResolvedSigner {
    pub fn check(
        _view: &AccountView,
        _field_name: &str,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

// ── Mode 2: struct with fields (typed zero-copy access) ──

#[account(custom)]
pub struct OraclePrice {
    pub price: u64,
    pub confidence: u64,
    pub slot: u64,
}

impl OraclePrice {
    pub fn check(
        view: &AccountView,
        _field_name: &str,
    ) -> Result<(), ProgramError> {
        // Custom validation — check data length, skip owner/disc
        let data = unsafe { view.borrow_unchecked() };
        if data.len() < core::mem::size_of::<u64>() * 3 {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }
}

// ── Use both in derive ──

#[derive(Accounts)]
pub struct ReadOracle {
    pub signer: ResolvedSigner,
    pub oracle: OraclePrice, // custom type used directly, not Account<OraclePrice>
}

#[program]
pub mod test_custom_account {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn read_oracle(ctx: Ctx<ReadOracle>) -> Result<(), ProgramError> {
        let _ = ctx.accounts.signer.to_account_view();
        // Typed access to oracle fields via zero-copy deref:
        let _price = ctx.accounts.oracle.price;
        let _conf = ctx.accounts.oracle.confidence;
        Ok(())
    }
}

fn main() {}
