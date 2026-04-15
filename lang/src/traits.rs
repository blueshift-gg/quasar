//! Core trait definitions for the Quasar framework.
//!
//! Traits fall into three categories:
//!
//! **Compile-time markers** — associate constant metadata with types:
//! - `Owner` — expected on-chain program owner for an account type
//! - `Id` — program address for a program marker type
//! - `Discriminator` — byte prefix for accounts and instructions
//! - `Space` — total byte size of an account's data (including discriminator)
//!
//! **Parsing and validation** — drive the account deserialization pipeline:
//! - `ParseAccounts` — parse and validate a set of accounts from raw
//!   `AccountView` slices
//! - `AccountCheck` — runtime validation hook called during parsing
//! - `CheckOwner` — verify an account's on-chain owner matches expectations
//! - `Owners` — declare valid on-chain owners for interface account types
//! - `AccountCount` — declare how many accounts a struct consumes
//!
//! **Access and dispatch** — provide uniform access to account data:
//! - `AsAccountView` — convert any account wrapper back to its raw
//!   `AccountView`
//! - `StaticView` — marks `#[repr(transparent)]` types safe for pointer cast
//!   construction
//! - `ZeroCopyDeref` — zero-copy `Deref`/`DerefMut` to `#[repr(C)]` account
//!   layouts
//! - `ProgramInterface` — check an address against multiple valid program IDs
//!
//! **Events** — `Event` supports dual emission (log-based and self-CPI).

use crate::prelude::{AccountView, Address, ProgramError};

/// Declares the expected on-chain owner for an account type.
///
/// Implemented by: `#[account]` derive macro.
/// Used by: `Account<T>` to validate owner during parsing.
pub trait Owner {
    const OWNER: Address;
}

/// Declares the on-chain address (ID) for a program type.
///
/// This trait simply provides the program's address constant. The `Program<T>`
/// wrapper type requires `T: Id` to validate that accounts match the expected
/// address.
///
/// Implemented by: Program marker types (e.g., `System`, `Token`).
/// Used by: `Program<T>` wrapper for address validation.
pub trait Id {
    const ID: Address;
}

/// Declares that a type represents a program interface accepting multiple
/// program IDs.
///
/// Unlike `Program` which represents a single program, this trait allows
/// checking against multiple valid program addresses (e.g., Token vs
/// Token-2022).
///
/// Implemented by: Manual `impl` for interface types.
/// Used by: `Interface<T>` to validate the address during parsing.
pub trait ProgramInterface {
    /// Check if the given address matches any valid program ID for this
    /// interface.
    fn matches(address: &Address) -> bool;
}

/// Declares the byte-level discriminator prefix for an account or instruction.
///
/// Discriminators are developer-specified (not sha256-derived). All-zero
/// discriminators are rejected at compile time to prevent uninitialized
/// account data from passing validation.
///
/// Implemented by: `#[account]` and `#[instruction]` derive macros.
pub trait Discriminator {
    const DISCRIMINATOR: &'static [u8];

    /// Byte offset of a stored `bump: u8` field from the start of account data.
    ///
    /// When `Some(offset)`, PDA validation can read the bump directly from
    /// account data and use `verify_program_address` (~200 CU) instead of
    /// `based_try_find_program_address` (~544 CU). Automatically set by
    /// `#[account]` when the struct contains a `bump: u8` field.
    const BUMP_OFFSET: Option<usize> = None;
}

/// Declares the total byte size of an account's data (including discriminator
/// prefix).
///
/// Implemented by: `#[account]` derive macro.
/// Used by: `create_account` CPI to allocate the correct account size.
pub trait Space {
    const SPACE: usize;
}

/// Runtime validation hook called during account parsing.
///
/// `check()` runs after owner/header validation and before pointer casts.
/// `validate()` is an optional parameterized hook used by namespaced
/// constraints (for example `token::mint`).
///
/// The default implementations are no-ops.
pub trait AccountCheck {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Per-type config for namespaced constraints.
    type Params: Default;

    #[inline(always)]
    fn validate(_view: &AccountView, _params: &Self::Params) -> Result<(), ProgramError> {
        Ok(())
    }
}

/// Declares the number of accounts consumed by a struct during parsing.
///
/// Implemented by: `#[derive(Accounts)]` macro.
/// Used by: `dispatch!` macro to size the `MaybeUninit` account buffer.
pub trait AccountCount {
    const COUNT: usize;
}

/// Parse and validate a set of accounts from a raw `AccountView` slice.
///
/// Implemented by: `#[derive(Accounts)]` macro.
/// Called by: `Ctx::new()` during instruction dispatch.
///
/// Returns the parsed struct and a `Bumps` companion containing any PDA bump
/// seeds discovered during validation.
pub trait ParseAccounts<'input>: Sized {
    type Bumps: Copy;
    fn parse(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError>;

    /// Parse accounts with access to instruction data.
    ///
    /// When `#[instruction(args)]` is present on the Accounts struct, the
    /// derived impl deserializes declared args from `data` and makes them
    /// available during account validation and initialization.
    ///
    /// The default implementation ignores `data` and delegates to `parse`.
    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'input mut [AccountView],
        _data: &[u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        Self::parse(accounts, program_id)
    }

    /// Set to `true` when overriding [`validate()`](Self::validate).
    /// The dispatch path uses this to skip the call entirely when no
    /// custom validation exists, avoiding a dead branch on sBPF.
    const HAS_VALIDATE: bool = false;

    /// User-defined validation hook called after all field-level checks pass
    /// but before the instruction handler executes.
    ///
    /// Override this to add cross-field validation that the `#[account(...)]`
    /// attribute DSL cannot express. The default implementation is a no-op.
    ///
    /// Lifecycle: `parse()` -> `validate()` -> handler -> `epilogue()`
    ///
    /// The signature is `&self` (not `&mut self`) — validation must not mutate
    /// validated account references. You must also set `const HAS_VALIDATE:
    /// bool = true;` for the hook to be called.
    #[inline(always)]
    fn validate(&self) -> Result<(), ProgramError> {
        Ok(())
    }

    #[inline(always)]
    fn epilogue(&mut self) -> Result<(), ProgramError> {
        Ok(())
    }
}

/// Internal exact-length parsing fast path used by dispatch and nested
/// composite account parsing.
///
/// # Safety
///
/// The caller must ensure `accounts.len() == Self::COUNT`.
#[doc(hidden)]
pub unsafe trait ParseAccountsUnchecked<'input>: ParseAccounts<'input> {
    /// # Safety
    ///
    /// `accounts.len()` must exactly match `Self::COUNT`.
    unsafe fn parse_unchecked(
        accounts: &'input mut [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError>;

    /// # Safety
    ///
    /// `accounts.len()` must exactly match `Self::COUNT`.
    #[inline(always)]
    unsafe fn parse_with_instruction_data_unchecked(
        accounts: &'input mut [AccountView],
        _data: &[u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        Self::parse_unchecked(accounts, program_id)
    }
}

/// Convert a typed account wrapper to its underlying [`AccountView`].
///
/// All account types (`Account<T>`, `Signer`, `UncheckedAccount`, etc.)
/// implement this trait to provide uniform access to the raw account data.
pub trait AsAccountView {
    fn to_account_view(&self) -> &AccountView;

    #[inline(always)]
    fn address(&self) -> &Address {
        self.to_account_view().address()
    }
}

impl AsAccountView for AccountView {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        self
    }
}

/// Validate that an account is owned by the expected program(s).
///
/// Single-owner types get a blanket implementation via [`Owner`].
/// Multi-owner (interface) types implement this directly with explicit
/// comparison chains, avoiding the ~20-40 CU cost of slice iteration.
pub trait CheckOwner {
    fn check_owner(view: &AccountView) -> Result<(), ProgramError>;
}

impl<T: Owner> CheckOwner for T {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!crate::keys_eq(view.owner(), &T::OWNER)) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

/// Declares the set of valid on-chain owners for interface account types.
///
/// Unlike [`Owner`] (single compile-time-known address), `Owners` returns
/// a static slice of valid owner addresses for runtime multi-owner checking.
///
/// Implemented by: Manual `impl` for SPL token/mint types in `quasar-spl`.
/// Used by: `InterfaceAccount<T>` to validate owner during parsing.
pub trait Owners {
    /// Static slice of valid owner program addresses.
    fn owners() -> &'static [Address];
}

/// Marker trait for account types with a `#[seeds]` definition.
/// Generated by the `#[account]` macro when `#[seeds(...)]` is present.
pub trait HasSeeds {
    /// The constant byte prefix for this PDA (e.g., b"vault").
    const SEED_PREFIX: &'static [u8];
    /// Number of dynamic seed arguments (excluding prefix and bump).
    const SEED_DYNAMIC_COUNT: usize;
}

/// Marker trait for account view types that are `#[repr(transparent)]` over
/// `AccountView` and therefore safe to construct via pointer cast.
///
/// # Safety
///
/// The implementor must be `#[repr(transparent)]` over `AccountView` (possibly
/// through a chain of transparent wrappers). This guarantees that a pointer
/// cast from `&AccountView` to `&Self` is sound.
///
/// Implemented by: `#[account]` macro for fixed-size accounts.
pub unsafe trait StaticView {}

/// Zero-copy deref target for `#[repr(C)]` account types.
///
/// When an account type implements `ZeroCopyDeref`, the wrapper provides
/// `Deref`/`DerefMut` to `T::Target` via `deref_from` / `deref_from_mut`.
///
/// Used by `InterfaceAccount<T>` for zero-copy field access.
///
/// Implemented by: `#[account]` macro (for SPL token/mint types).
pub trait ZeroCopyDeref {
    type Target;

    /// # Safety
    ///
    /// The caller must ensure `view.data_len()` is large enough for the
    /// zero-copy target and that the underlying bytes match the implementor's
    /// layout contract.
    unsafe fn deref_from(view: &AccountView) -> &Self::Target;

    #[allow(clippy::mut_from_ref)]
    /// # Safety
    ///
    /// Same requirements as [`deref_from`](Self::deref_from), plus the caller
    /// must ensure the account is writable and there are no conflicting
    /// aliases.
    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target;
}

/// On-chain event with a discriminator, fixed-size data, and emission logic.
///
/// Events support dual emission:
/// - `emit!()` via `sol_log_data` (~100 CU) — fast but spoofable
/// - `emit_cpi!()` via self-CPI (~1,000 CU) — spoofing-resistant
///
/// Implemented by: `#[event]` derive macro.
pub trait Event {
    const DISCRIMINATOR: &'static [u8];
    const DATA_SIZE: usize;
    fn write_data(&self, buf: &mut [u8]);
    fn emit(&self, f: impl FnOnce(&[u8]) -> Result<(), ProgramError>) -> Result<(), ProgramError>;
}

/// Verify that the actual account count matches the expected count.
///
/// Used by `ParseAccounts` / `ParseAccountsUnchecked` impls to reject
/// instruction calls with wrong account counts.
#[inline(always)]
pub fn check_account_count(actual: usize, expected: usize) -> Result<(), ProgramError> {
    if crate::utils::hint::unlikely(actual != expected) {
        return Err(if actual < expected {
            ProgramError::NotEnoughAccountKeys
        } else {
            ProgramError::InvalidArgument
        });
    }
    Ok(())
}
