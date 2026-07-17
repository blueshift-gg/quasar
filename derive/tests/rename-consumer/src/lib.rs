//! Rename / no-glob smoke test.
//!
//! This crate depends on `quasar-lang` under the renamed package `ql` and does
//! NOT `use ql::prelude::*`. Only author-facing names are imported; the trait
//! and type names that *only generated code* references are deliberately left
//! out. A compile failure here means the derive emitted a `quasar_lang::` path
//! (broken by the rename) or a bare prelude name (broken without the glob).
#![no_std]
#![allow(dead_code, unexpected_cfgs, unused_imports)]

// Author-facing macros + wrapper types the program *writes*, plus the traits
// whose *methods* the generated code invokes on `self` (e.g. `.to_account_view()`,
// `Self::COUNT`, `.epilogue()`). Trait-method resolution needs the trait in
// scope; generated code fully qualifies every trait/type *name* it references,
// so nothing else from the prelude is imported (no glob).
use ql::prelude::{
    account, declare_id, emit_cpi, error_code, event, instruction, program, Account,
    AccountCount, Accounts, Address, AsAccountView, Ctx, CtxWithRemaining, ParseAccounts,
    Program, ProgramError, Signer, String, SystemProgram,
};

declare_id!("11111111111111111111111111111111111111111111");

#[account(discriminator = 1, set_inner)]
#[seeds(b"vault", authority: Address)]
pub struct Vault {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = 2, set_inner)]
pub struct DynVault {
    pub authority: Address,
    pub label: String<16>,
}

#[event(discriminator = 1)]
pub struct Moved {
    pub amount: u64,
}

#[error_code]
pub enum RenameError {
    Unauthorized,
    BadValue,
}

#[derive(Accounts)]
pub struct InitVault {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, address = Vault::seeds(payer.address()))]
    pub account: Account<Vault>,
    pub system_program: Program<SystemProgram>,
}
impl InitVault {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CloseVault {
    #[account(mut)]
    pub authority: Signer,
    #[account(
        mut,
        has_one(authority),
        close(dest = authority),
        address = Vault::seeds(authority.address()),
    )]
    pub account: Account<Vault>,
}
impl CloseVault {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitMoved {
    pub signer: Signer,
    pub event_authority: EventAuthority,
    pub program: Program<RenameConsumerProgram>,
}
impl EmitMoved {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        emit_cpi!(Moved { amount })?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DynIx {
    #[account(mut)]
    pub account: Account<DynVault>,
}
impl DynIx {
    #[inline(always)]
    pub fn handler(&self, _label: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct WithRemaining {
    pub signer: Signer,
}
impl WithRemaining {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[program]
mod rename_consumer {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn init_vault(ctx: Ctx<InitVault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 1)]
    pub fn close_vault(ctx: Ctx<CloseVault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn emit_moved(ctx: Ctx<EmitMoved>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 3)]
    pub fn dyn_ix(ctx: Ctx<DynIx>, label: String<16>) -> Result<(), ProgramError> {
        ctx.accounts.handler(label)
    }

    #[instruction(discriminator = 4)]
    pub fn with_remaining(ctx: CtxWithRemaining<WithRemaining>) -> Result<(), ProgramError> {
        let _ = ctx.remaining_accounts();
        ctx.accounts.handler()
    }
}
