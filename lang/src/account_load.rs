//! Validation and construction for account wrapper types.
//!
//! [`AccountLoad`](crate::account_load::AccountLoad) turns a validated
//! [`AccountView`](solana_account_view::AccountView) into a typed wrapper.
//! Every wrapper is `#[repr(transparent)]` over `AccountView` — enforced by
//! the [`StaticView`](crate::traits::StaticView) supertrait — so each loader
//! validates the account first, then constructs the wrapper with a pointer
//! cast relying on that layout.

use {
    crate::traits::{AsAccountView, StaticView},
    solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Unified validation, construction, and header flag consts for account wrapper
/// types.
///
/// All implementors must be `#[repr(transparent)]` over `AccountView`
/// (possibly through a chain of transparent wrappers). The [`StaticView`]
/// supertrait makes that requirement a compile-time obligation: the
/// pointer-cast constructors below are only sound because every implementor is
/// layout-compatible with `AccountView`.
///
/// # Validator selection
///
/// The derive picks one validator per field (see
/// `derive/src/accounts/emit/parse.rs`), trading validation for CU:
///
/// | Field shape | Loader | Validator |
/// |-------------|--------|-----------|
/// | default (unique field) | `load` / `load_mut` | [`check`](Self::check) — unchecked data borrow (the field is unique, so no aliasing) |
/// | `#[account(dup)]` | `load_checked` / `load_mut_checked` | [`check_checked`](Self::check_checked) — runtime-checked borrow, sound under aliasing |
/// | a behavior sets `VALIDATES_ACCOUNT_DATA` | `load_intrinsic` / `load_mut_intrinsic` (**unsafe**) | [`check_intrinsic`](Self::check_intrinsic) — intrinsic invariants only; the behavior re-validates the data |
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a loadable account type",
    label = "not an account wrapper",
    note = "wrap the field in `Account<T>`, or add `#[account]` to the struct you are trying to \
            load"
)]
pub trait AccountLoad: AsAccountView + StaticView + Sized {
    /// Whether the account header must mark this account as a signer.
    const IS_SIGNER: bool = false;
    /// Whether the account header must mark this account as executable.
    const IS_EXECUTABLE: bool = false;

    /// Validate this account after header flag checks pass.
    ///
    /// Header flags (signer, writable, executable) are already validated by
    /// `parse_accounts` before this is called.
    fn check(view: &AccountView) -> Result<(), ProgramError>;

    /// Validate through runtime-checked account-data borrows.
    ///
    /// The default implementation is equivalent to [`Self::check`] for account
    /// wrappers that do not inspect data. Data-bearing account types override
    /// this so `#[account(dup)]` fields validate without unchecked aliasing.
    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        Self::check(view)
    }

    /// Validate only intrinsic account invariants (owner, discriminator),
    /// skipping the data-schema validation a following behavior will perform.
    ///
    /// The default preserves normal account loading (equivalent to
    /// [`Self::check`]). Data-bearing wrappers may override it to skip the
    /// schema check. Because the returned account is not fully validated, the
    /// only loaders that call it — [`Self::load_intrinsic`] /
    /// [`Self::load_mut_intrinsic`] — are `unsafe`, and the derive emits them
    /// only when a behavior with `VALIDATES_ACCOUNT_DATA` re-validates the data
    /// before it is observed.
    #[inline(always)]
    fn check_intrinsic(view: &AccountView) -> Result<(), ProgramError> {
        Self::check(view)
    }

    #[inline(always)]
    /// Validates and loads an immutable account wrapper.
    fn load(view: &AccountView) -> Result<Self, ProgramError> {
        Self::check(view)?;
        // SAFETY: `check` validated the account; `Self: StaticView` makes it
        // layout-compatible with `AccountView`, so the pointer read is sound.
        Ok(unsafe { core::ptr::read(view as *const AccountView as *const Self) })
    }

    #[inline(always)]
    /// Validates and loads a mutable account wrapper.
    fn load_mut(view: &mut AccountView) -> Result<Self, ProgramError> {
        Self::check(view)?;
        // SAFETY: `check` validated the account and `Self: StaticView` gives
        // layout compatibility; mutable load runs only after generated
        // writable checks.
        Ok(unsafe { core::ptr::read(view as *mut AccountView as *const Self) })
    }

    #[inline(always)]
    /// Validates and loads through runtime-checked data borrows.
    fn load_checked(view: &AccountView) -> Result<Self, ProgramError> {
        Self::check_checked(view)?;
        // SAFETY: `check_checked` validated the account through runtime-checked
        // borrows; `Self: StaticView` gives layout compatibility with
        // `AccountView`.
        Ok(unsafe { core::ptr::read(view as *const AccountView as *const Self) })
    }

    #[inline(always)]
    /// Mutably loads through runtime-checked data borrows.
    fn load_mut_checked(view: &mut AccountView) -> Result<Self, ProgramError> {
        Self::check_checked(view)?;
        // SAFETY: `check_checked` validated the account and `Self: StaticView`
        // gives layout compatibility; mutable load runs only after generated
        // writable checks.
        Ok(unsafe { core::ptr::read(view as *mut AccountView as *const Self) })
    }

    /// # Safety
    ///
    /// Caller must ensure any validation skipped by `check_intrinsic` is
    /// completed before the loaded account can be observed or dereferenced.
    #[inline(always)]
    unsafe fn load_intrinsic(view: &AccountView) -> Result<Self, ProgramError> {
        Self::check_intrinsic(view)?;
        // SAFETY: Caller guarantees validation skipped by `check_intrinsic`
        // will be completed before observation.
        Ok(unsafe { core::ptr::read(view as *const AccountView as *const Self) })
    }

    /// # Safety
    ///
    /// Caller must ensure any validation skipped by `check_intrinsic` is
    /// completed before the loaded account can be observed or dereferenced.
    #[inline(always)]
    unsafe fn load_mut_intrinsic(view: &mut AccountView) -> Result<Self, ProgramError> {
        Self::check_intrinsic(view)?;
        // SAFETY: Caller guarantees validation skipped by `check_intrinsic`
        // will be completed before observation; mutable load is only used
        // after generated writable checks.
        Ok(unsafe { core::ptr::read(view as *mut AccountView as *const Self) })
    }

    /// Get a mutable view for lifecycle operations (close, realloc).
    ///
    /// # Safety
    ///
    /// Caller must ensure the account is writable and that no other
    /// references to the underlying `AccountView` are live. Only called
    /// by generated epilogue code after writable/lifecycle checks pass.
    #[doc(hidden)]
    #[inline(always)]
    unsafe fn to_account_view_mut(&mut self) -> &mut AccountView {
        // SAFETY: Caller guarantees exclusive access to the wrapper and that
        // `Self` is layout-compatible with `AccountView`.
        unsafe { &mut *(self as *mut Self as *mut AccountView) }
    }
}
