//! Instruction context types used by the `dispatch!` macro.
//!
//! Three levels of context exist, each wrapping the previous:
//!
//! - `Context` — raw entrypoint data (program ID, account slice, instruction
//!   data). Produced by the entrypoint; consumed by `Ctx::new()` or
//!   `CtxWithRemaining::new()`.
//!
//! - `Ctx` — parsed and validated accounts with PDA bumps. Use this for most
//!   instructions where remaining accounts are not needed.
//!
//! - `CtxWithRemaining` — like `Ctx` but also captures the remaining accounts
//!   region for instructions that forward accounts to CPIs (e.g., token
//!   transfers with extra signers or route swaps).

use crate::{prelude::*, remaining::RemainingAccounts};

/// Cast `&[u8; 32]` to `&Address`. Address is `#[repr(transparent)]` over `[u8;
/// 32]`.
#[inline(always)]
unsafe fn as_address(bytes: &[u8; 32]) -> &Address {
    &*(bytes as *const [u8; 32] as *const Address)
}

/// Raw entrypoint context before parsing.
///
/// Produced by the `dispatch!` macro from the entrypoint's raw pointers.
/// Consumed by [`Ctx::new()`] or [`CtxWithRemaining::new()`] which parse
/// and validate the accounts.
pub struct Context<'info> {
    /// 32-byte program ID passed by the runtime.
    pub program_id: &'info [u8; 32],

    /// Declared accounts (first `N` accounts deserialized from the input).
    pub accounts: &'info mut [AccountView],

    /// Pointer to the first remaining account (past the declared accounts).
    pub remaining_ptr: *mut u8,

    /// Raw instruction data (discriminator already consumed by `dispatch!`).
    pub data: &'info [u8],

    /// End of accounts region: `ix_data_ptr - sizeof(u64)`.
    pub accounts_boundary: *const u8,
}

/// Parsed instruction context with typed accounts and PDA bumps.
///
/// Use [`CtxWithRemaining`] for instructions that need
/// `remaining_accounts()`.
pub struct Ctx<'info, T: ParseAccounts<'info> + AccountCount> {
    /// Validated and typed account struct.
    pub accounts: T,

    /// PDA bump seeds discovered during validation.
    pub bumps: T::Bumps,

    /// 32-byte program ID (raw bytes, not [`Address`]).
    pub program_id: &'info [u8; 32],

    /// Instruction data with discriminator already consumed.
    pub data: &'info [u8],
}

impl<'info, T: ParseAccounts<'info> + AccountCount> Ctx<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        let (accounts, bumps) =
            T::parse_with_instruction_data(ctx.accounts, ctx.data, program_id_addr)?;
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
        })
    }
}

/// Like [`Ctx`] but also captures the remaining accounts region.
///
/// Use this for instructions that call `remaining_accounts()` — e.g.
/// token transfers with extra signers, route swaps, or any CPI that
/// forwards a variable number of accounts.
pub struct CtxWithRemaining<'info, T: ParseAccounts<'info> + AccountCount> {
    /// Validated and typed account struct.
    pub accounts: T,

    /// PDA bump seeds discovered during validation.
    pub bumps: T::Bumps,

    /// 32-byte program ID (raw bytes).
    pub program_id: &'info [u8; 32],

    /// Instruction data with discriminator already consumed.
    pub data: &'info [u8],

    /// Pointer to the first remaining account in the input buffer.
    remaining_ptr: *mut u8,

    /// Declared accounts slice (for duplicate resolution in remaining).
    declared: &'info [AccountView],

    /// End-of-accounts boundary pointer.
    accounts_boundary: *const u8,
}

impl<'info, T: ParseAccounts<'info> + AccountCount> CtxWithRemaining<'info, T> {
    #[inline(always)]
    pub fn new(ctx: Context<'info>) -> Result<Self, ProgramError> {
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        // Save slice metadata before parse consumes the &mut borrow.
        // Safety: AccountView is Copy and values are stable after parsing.
        // The declared slice is only used for read-only duplicate resolution.
        let declared_ptr = ctx.accounts.as_ptr();
        let declared_len = ctx.accounts.len();
        let (accounts, bumps) =
            T::parse_with_instruction_data(ctx.accounts, ctx.data, program_id_addr)?;
        let declared = unsafe { core::slice::from_raw_parts(declared_ptr, declared_len) };
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining_ptr: ctx.remaining_ptr,
            declared,
            accounts_boundary: ctx.accounts_boundary,
        })
    }

    #[inline(always)]
    pub fn remaining_accounts(&self) -> RemainingAccounts<'info> {
        RemainingAccounts::new(self.remaining_ptr, self.accounts_boundary, self.declared)
    }
}
