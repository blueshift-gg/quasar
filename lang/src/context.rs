//! Instruction context types used by the generated `#[program]` dispatch.
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

use crate::{
    prelude::*,
    remaining::{Remaining, RemainingAccounts, RemainingItem},
    traits::ParseAccountsUnchecked,
};

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
    /// Private: only entrypoint codegen can supply a valid value, and safe
    /// methods dereference it.
    remaining_ptr: *mut u8,

    /// Raw instruction data (discriminator already consumed by dispatch).
    pub data: &'input [u8],

    /// End of accounts region: `ix_data_ptr - sizeof(u64)`. Private for the
    /// same reason as `remaining_ptr`.
    accounts_boundary: *const u8,
}

impl<'input> Context<'input> {
    /// Assemble a `Context` from raw entrypoint parts.
    ///
    /// # Safety
    ///
    /// `remaining_ptr` must point at the first remaining account entry of the
    /// SVM input buffer the other parts came from (or at `accounts_boundary`
    /// when there are none), and `accounts_boundary` must be the end of that
    /// buffer's account region. Safe code derives `RemainingAccounts` walks
    /// from these pointers without further checks.
    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn from_raw_parts(
        program_id: &'input [u8; 32],
        accounts: &'input mut [AccountView],
        data: &'input [u8],
        remaining_ptr: *mut u8,
        accounts_boundary: *const u8,
    ) -> Self {
        Self {
            program_id,
            accounts,
            remaining_ptr,
            data,
            accounts_boundary,
        }
    }
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
    ///
    /// Length-checked: rejects a `Context` whose account slice does not hold
    /// `T::COUNT` accounts instead of exhibiting undefined behavior. Generated
    /// dispatch code uses [`Ctx::new_unchecked`] because the entrypoint parser
    /// has already proven the count.
    #[inline(always)]
    pub fn new(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at the runtime-owned 32-byte
        // program id for this instruction.
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

    /// Parse without the account-count check.
    ///
    /// # Safety
    ///
    /// `ctx.accounts` must hold exactly `T::COUNT` validated account views, as
    /// produced by the generated entrypoint parser.
    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn new_unchecked(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at the runtime-owned 32-byte
        // program id for this instruction.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        // SAFETY: The caller guarantees `ctx.accounts` holds exactly
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

/// Marker used by the raw, dynamically shaped remaining-account context.
///
/// Programs normally refer to it through the default
/// `CtxWithRemaining<Accounts>` parameters rather than naming it directly.
#[doc(hidden)]
pub struct DynamicRemaining;

/// Like [`Ctx`] but also captures the remaining accounts region.
///
/// `CtxWithRemaining<Accounts>` preserves the raw compatibility API for
/// intentionally dynamic or forwarded tails. Prefer
/// `CtxWithRemaining<Accounts, Item, N>` when the handler consumes a known
/// account type: dispatch validates the complete tail and exposes it through
/// [`Self::remaining`].
pub struct CtxWithRemaining<
    'input,
    T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
    R = DynamicRemaining,
    const N: usize = 0,
> {
    /// Validated and typed account struct.
    pub accounts: T,

    /// PDA bump seeds discovered during validation.
    pub bumps: <T as ParseAccounts<'input>>::Bumps,

    /// 32-byte program ID (raw bytes).
    pub program_id: &'input [u8; 32],

    /// Instruction data with discriminator already consumed.
    pub data: &'input [u8],

    /// Parsed bounded remaining accounts.
    ///
    /// This is an empty internal value for the dynamic one-parameter form;
    /// call [`Self::remaining_accounts`] on that form instead.
    pub remaining: Remaining<R, N>,

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
    ///
    /// Length-checked: rejects a `Context` whose account slice does not hold
    /// `T::COUNT` accounts. Generated dispatch code uses
    /// [`CtxWithRemaining::new_unchecked`].
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
        let (accounts, bumps) =
            T::parse_with_instruction_data(ctx.accounts, ctx.data, program_id_addr)?;
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
            remaining: Remaining::empty(),
            remaining_ptr: ctx.remaining_ptr,
            declared,
            accounts_boundary: ctx.accounts_boundary,
        })
    }

    /// Parse without the account-count check.
    ///
    /// # Safety
    ///
    /// `ctx.accounts` must hold exactly `T::COUNT` validated account views, as
    /// produced by the generated entrypoint parser.
    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn new_unchecked(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at the runtime-owned 32-byte
        // program id for this instruction.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        // Save pointer + length before parse consumes the mutable slice (see
        // `new` for the borrow rationale).
        let declared_ptr = ctx.accounts.as_ptr();
        let declared_len = ctx.accounts.len();
        // SAFETY: The caller guarantees `ctx.accounts` holds exactly
        // `T::COUNT` declared account views for this handler.
        let (accounts, bumps) = unsafe {
            T::parse_with_instruction_data_unchecked(ctx.accounts, ctx.data, program_id_addr)?
        };
        // SAFETY: Same aliasing argument as `new`: the shared slice is only
        // read for address comparisons and does not conflict with the parsed
        // struct's &mut refs into RuntimeAccount data under Tree Borrows.
        let declared = unsafe { core::slice::from_raw_parts(declared_ptr, declared_len) };
        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining: Remaining::empty(),
            remaining_ptr: ctx.remaining_ptr,
            declared,
            accounts_boundary: ctx.accounts_boundary,
        })
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

impl<
        'input,
        T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
        R: RemainingItem<'input>,
        const N: usize,
    > CtxWithRemaining<'input, T, R, N>
{
    /// Parse declared accounts and a complete bounded remaining-account tail.
    #[inline(always)]
    pub fn new_typed(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at runtime-owned storage.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        let declared_ptr = ctx.accounts.as_ptr();
        let declared_len = ctx.accounts.len();
        let (accounts, bumps) =
            T::parse_with_instruction_data(ctx.accounts, ctx.data, program_id_addr)?;
        // SAFETY: Parsing copies views from this still-live slice. See the raw
        // constructor above for the aliasing argument.
        let declared = unsafe { core::slice::from_raw_parts(declared_ptr, declared_len) };
        // SAFETY: The context owns matching pointers into the same SVM input
        // buffer and the matching declared-account prefix.
        let raw = unsafe {
            RemainingAccounts::new_with_context(
                ctx.remaining_ptr,
                ctx.accounts_boundary,
                declared,
                program_id_addr,
                ctx.data,
            )
        };
        let remaining = raw.parse::<R, N>()?;

        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining,
            remaining_ptr: ctx.remaining_ptr,
            declared,
            accounts_boundary: ctx.accounts_boundary,
        })
    }

    /// Parse a bounded tail without repeating the declared-account count
    /// check already performed by generated dispatch.
    ///
    /// # Safety
    ///
    /// `ctx.accounts` must hold exactly `T::COUNT` validated account views.
    #[doc(hidden)]
    #[inline(always)]
    pub unsafe fn new_typed_unchecked(ctx: Context<'input>) -> Result<Self, ProgramError> {
        // SAFETY: `Context::program_id` points at runtime-owned storage.
        let program_id_addr = unsafe { as_address(ctx.program_id) };
        let declared_ptr = ctx.accounts.as_ptr();
        let declared_len = ctx.accounts.len();
        // SAFETY: The caller guarantees the declared account count.
        let (accounts, bumps) = unsafe {
            T::parse_with_instruction_data_unchecked(ctx.accounts, ctx.data, program_id_addr)?
        };
        // SAFETY: Same live-slice argument as `new_typed`.
        let declared = unsafe { core::slice::from_raw_parts(declared_ptr, declared_len) };
        // SAFETY: The context owns matching pointers into the same input.
        let raw = unsafe {
            RemainingAccounts::new_with_context(
                ctx.remaining_ptr,
                ctx.accounts_boundary,
                declared,
                program_id_addr,
                ctx.data,
            )
        };
        let remaining = raw.parse::<R, N>()?;

        Ok(Self {
            accounts,
            bumps,
            program_id: ctx.program_id,
            data: ctx.data,
            remaining,
            remaining_ptr: ctx.remaining_ptr,
            declared,
            accounts_boundary: ctx.accounts_boundary,
        })
    }
}

impl<
        'input,
        T: ParseAccounts<'input> + ParseAccountsUnchecked<'input> + AccountCount,
        R,
        const N: usize,
    > CtxWithRemaining<'input, T, R, N>
{
    /// Compile-time check for whether `T` has lifecycle operations
    /// (close/sweep/migrate). When false, the epilogue call is elided.
    #[inline(always)]
    pub const fn has_epilogue(&self) -> bool {
        T::HAS_EPILOGUE
    }
}
