//! Instruction context types used by the `dispatch!` macro.
//!
//! Three levels of context exist, each wrapping the previous:
//!
//! - `Context`: raw entrypoint data (program ID, account slice, instruction
//!   data). Produced by the entrypoint; consumed by `Ctx::new()` or
//!   `CtxWithRemaining::new()`.
//!
//! - `Ctx`: parsed and validated accounts with PDA bumps. Use this for most
//!   instructions where remaining accounts are not needed.
//!
//! - `CtxWithRemaining`: like `Ctx` but also captures the remaining accounts
//!   region for instructions that inspect or forward trailing accounts.

use crate::{prelude::*, remaining::RemainingAccounts, traits::ParseAccountsUnchecked};

/// Cast `&[u8; 32]` to `&Address`.
///
/// The entrypoint owns the 32-byte program-id storage for the entire
/// instruction, so the returned reference is valid for `'input`. This avoids
/// copying the program ID into a stack-local `Address` on every dispatch path.
#[inline(always)]
unsafe fn as_address(bytes: &[u8; 32]) -> &Address {
    // SAFETY: `Address` is a transparent 32-byte address type and `bytes`
    // lives for the returned reference.
    unsafe { &*(bytes as *const [u8; 32] as *const Address) }
}

/// Raw entrypoint context before parsing.
///
/// Produced by entrypoint code from raw account pointers.
/// Consumed by [`Ctx::new()`] or [`CtxWithRemaining::new()`] which parse
/// and validate the accounts.
pub struct Context<'input> {
    /// 32-byte program ID passed by the runtime.
    pub program_id: &'input [u8; 32],

    /// Declared accounts (first `N` accounts deserialized from the input).
    pub accounts: &'input mut [AccountView],

    /// Pointer to the first remaining account (past the declared accounts).
    pub remaining_ptr: *mut u8,

    /// Raw instruction data (discriminator already consumed by `dispatch!`).
    pub data: &'input [u8],

    /// End of accounts region: `ix_data_ptr - sizeof(u64)`.
    pub accounts_boundary: *const u8,
}

/// Parsed instruction context with typed accounts and PDA bumps.
///
/// Use [`CtxWithRemaining`] for instructions that need
/// `remaining_accounts()`.
pub struct Ctx<'input, T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount> {
    /// Validated and typed account struct.
    pub accounts: T,

    /// PDA bump seeds discovered during validation.
    pub bumps: <T as ParseAccounts<'input>>::Bumps,

    /// 32-byte program ID (raw bytes, not [`Address`]).
    pub program_id: &'input [u8; 32],

    /// Instruction data with discriminator already consumed.
    pub data: &'input [u8],
}

impl<'input, T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount>
    Ctx<'input, T>
{
    /// Parse and validate the declared accounts for an instruction that does
    /// not expose remaining accounts.
    #[inline(always)]
    pub fn new(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at the runtime-owned 32-byte
        // program id for this instruction.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        // SAFETY: Entrypoint code constructed `ctx.accounts` with exactly
        // `T::COUNT` declared account views for this handler.
        let (accounts, bumps) = unsafe {
            T::parse_with_instruction_data_unchecked(ctx.accounts, ctx.data, program_id_addr)?
        };
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
        })
    }

    /// Compile-time check for whether `T` has lifecycle operations
    /// (close/sweep/migrate). When false, the epilogue call is elided.
    #[inline(always)]
    pub const fn has_epilogue(&self) -> bool {
        T::HAS_EPILOGUE
    }
}

/// Like [`Ctx`] but also captures the remaining accounts region.
///
/// Use this for instructions that call `remaining_accounts()`, for example when
/// inspecting trailing accounts in local logic or forwarding a variable number
/// of accounts to a downstream CPI.
pub struct CtxWithRemaining<
    'input,
    T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
> {
    /// Validated and typed account struct.
    pub accounts: T,

    /// PDA bump seeds discovered during validation.
    pub bumps: <T as ParseAccounts<'input>>::Bumps,

    /// 32-byte program ID (raw bytes).
    pub program_id: &'input [u8; 32],

    /// Instruction data with discriminator already consumed.
    pub data: &'input [u8],

    /// Pointer to the first remaining account in the input buffer.
    remaining_ptr: *mut u8,

    /// Declared accounts slice (for duplicate resolution in remaining).
    declared: &'input [AccountView],

    /// End-of-accounts boundary pointer.
    accounts_boundary: *const u8,
}

impl<'input, T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount>
    CtxWithRemaining<'input, T>
{
    /// Parse and validate declared accounts while preserving access to the
    /// trailing remaining-account region.
    #[inline(always)]
    pub fn new(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at the runtime-owned 32-byte
        // program id for this instruction.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        // Save pointer + length before parse consumes the mutable slice. This
        // avoids creating a shared declared-account slice while parsing still
        // holds the `&mut [AccountView]` borrow.
        let declared_ptr = ctx.accounts.as_ptr();
        let declared_len = ctx.accounts.len();
        // SAFETY: Entrypoint code constructed `ctx.accounts` with exactly
        // `T::COUNT` declared account views for this handler.
        let (accounts, bumps) = unsafe {
            T::parse_with_instruction_data_unchecked(ctx.accounts, ctx.data, program_id_addr)?
        };
        // SAFETY: The backing memory is still valid. Parse copies AccountView
        // values out of the slice, it does not deallocate. We construct the
        // shared slice here only for the RemainingAccounts API which uses it
        // for read-only address comparisons via keys_eq. The parsed struct
        // holds &mut refs into the pointed-to RuntimeAccount data (through
        // the raw pointers inside each AccountView), not into the AccountView
        // slice itself, so under Tree Borrows the shared slice's "Frozen"
        // permission on the AccountView pointer fields does not conflict.
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

    /// Compile-time check for whether `T` has lifecycle operations
    /// (close/sweep/migrate). When false, the epilogue call is elided.
    #[inline(always)]
    pub const fn has_epilogue(&self) -> bool {
        T::HAS_EPILOGUE
    }

    /// Remaining-account accessor.
    ///
    /// Preserves duplicate account metas exactly as they appeared in the input.
    /// Data access through yielded remaining-account handles uses checked
    /// runtime borrows, so duplicate entries are safe by default.
    #[inline(always)]
    pub fn remaining_accounts(&self) -> RemainingAccounts<'input> {
        // SAFETY: `remaining_ptr`/`accounts_boundary` delimit the remaining
        // region of the SVM input buffer this `CtxWithRemaining` was built from,
        // and `declared` is the declared-account slice parsed from that same
        // buffer, so the `RemainingAccounts` construction contract is upheld.
        // `program_id` is the same runtime-owned 32-byte storage originally
        // passed through `Context`.
        unsafe {
            RemainingAccounts::new_with_context(
                self.remaining_ptr,
                self.accounts_boundary,
                self.declared,
                as_address(self.program_id),
                self.data,
            )
        }
    }
}
