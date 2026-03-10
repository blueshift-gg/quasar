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
//! - `ParseAccounts` — parse and validate a set of accounts from raw `AccountView` slices
//! - `FromAccountView` — construct a single typed wrapper from an `AccountView`
//! - `AccountCheck` — runtime validation hook called during parsing
//! - `CheckOwner` — verify an account's on-chain owner matches expectations
//! - `AccountCount` — declare how many accounts a struct consumes
//!
//! **Access and dispatch** — provide uniform access to account data:
//! - `AsAccountView` — convert any account wrapper back to its raw `AccountView`
//! - `StaticView` — marks `#[repr(transparent)]` types safe for pointer cast construction
//! - `ZeroCopyDeref` — zero-copy `Deref`/`DerefMut` to `#[repr(C)]` account layouts
//! - `InterfaceResolve` — polymorphic dispatch for multi-program interface accounts
//! - `ProgramInterface` — check an address against multiple valid program IDs
//!
//! **Events** — `Event` supports dual emission (log-based and self-CPI).

use crate::prelude::AccountView;
use crate::prelude::Address;
use crate::prelude::ProgramError;

/// Construct a typed account wrapper from a raw [`AccountView`].
///
/// Implemented by the `define_account!` macro and by `Account<T>`.
pub trait FromAccountView<'info>: Sized {
    fn from_account_view(view: &'info AccountView) -> Result<Self, ProgramError>;
}

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
/// wrapper type requires `T: Id` to validate that accounts match the expected address.
///
/// Implemented by: Program marker types (e.g., `System`, `Token`).
/// Used by: `Program<T>` wrapper for address validation.
pub trait Id {
    const ID: Address;
}

/// Declares that a type represents a program interface accepting multiple program IDs.
///
/// Unlike `Program` which represents a single program, this trait allows
/// checking against multiple valid program addresses (e.g., Token vs Token-2022).
///
/// Implemented by: Manual `impl` for interface types.
/// Used by: `Interface<T>` to validate the address during parsing.
pub trait ProgramInterface {
    /// Check if the given address matches any valid program ID for this interface.
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
}

/// Declares the total byte size of an account's data (including discriminator prefix).
///
/// Implemented by: `#[account]` derive macro.
/// Used by: `create_account` CPI to allocate the correct account size.
pub trait Space {
    const SPACE: usize;
}

/// Runtime validation hook called during account parsing.
///
/// The default implementation is a no-op. Override to add custom checks
/// (e.g. verifying a mint address or checking account state).
///
/// Implemented by: `define_account!` macro (for check trait composition),
/// manual `impl` for custom validation.
pub trait AccountCheck {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
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
pub trait ParseAccounts<'info>: Sized {
    type Bumps: Copy;
    fn parse(
        accounts: &'info [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError>;

    /// Parse accounts with access to instruction data.
    ///
    /// When `#[instruction(args)]` is present on the Accounts struct, the
    /// derived impl deserializes declared args from `data` and makes them
    /// available during account initialization (e.g. for `metadata::name`).
    ///
    /// The default implementation ignores `data` and delegates to `parse`.
    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'info [AccountView],
        _data: &'info [u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        Self::parse(accounts, program_id)
    }

    #[inline(always)]
    fn epilogue(&self) -> Result<(), ProgramError> {
        Ok(())
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
        if !crate::keys_eq(unsafe { view.owner() }, &T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

/// Polymorphic zero-copy dispatch for interface account types.
///
/// When an account can be owned by multiple programs with different layouts
/// or behaviors, `InterfaceResolve` provides a way to dispatch to the
/// correct resolved type based on the runtime owner.
pub trait InterfaceResolve {
    type Resolved<'a>;
    fn resolve<'a>(view: &'a AccountView) -> Result<Self::Resolved<'a>, ProgramError>;
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
    fn deref_from(view: &AccountView) -> &Self::Target;
    #[allow(clippy::mut_from_ref)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target;
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
