use core::marker::PhantomData;

use quasar_core::prelude::*;

use crate::constants::{SPL_TOKEN_ID, TOKEN_2022_ID};
use crate::cpi::TokenCpi;

/// Generic interface account wrapper — accepts accounts owned by either
/// SPL Token or Token-2022.
///
/// `InterfaceAccount<T>` is a peer to `Account<T>`. Where `Account<Token>`
/// only accepts SPL Token-owned accounts, `InterfaceAccount<Token>` accepts
/// both SPL Token and Token-2022. The inner marker `T` provides the data
/// layout check and zero-copy deref target.
///
/// ```ignore
/// pub vault: &'info InterfaceAccount<Token>,
/// pub mint: &'info InterfaceAccount<Mint>,
/// ```
#[repr(transparent)]
pub struct InterfaceAccount<T> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T> AsAccountView for InterfaceAccount<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: AccountCheck> InterfaceAccount<T> {
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        let owner = unsafe { view.owner() };
        if !quasar_core::keys_eq(owner, &SPL_TOKEN_ID)
            && !quasar_core::keys_eq(owner, &TOKEN_2022_ID)
        {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    /// # Safety (invalid_reference_casting)
    ///
    /// `Self` is `#[repr(transparent)]` over `AccountView`, which uses
    /// interior mutability through raw pointers to SVM account memory.
    /// The `&` → `&mut` cast does not create aliased mutable references;
    /// all writes go through `AccountView`'s raw pointer methods.
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        let owner = unsafe { view.owner() };
        if !quasar_core::keys_eq(owner, &SPL_TOKEN_ID)
            && !quasar_core::keys_eq(owner, &TOKEN_2022_ID)
        {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

impl<T: ZeroCopyDeref> core::ops::Deref for InterfaceAccount<T> {
    type Target = T::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        T::deref_from(&self.view)
    }
}

impl<T: ZeroCopyDeref> core::ops::DerefMut for InterfaceAccount<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        T::deref_from_mut(&self.view)
    }
}

impl<T: InterfaceResolve> InterfaceAccount<T> {
    /// Dispatch to a program-specific resolved type based on the runtime owner.
    ///
    /// The owner check ran once during account parsing. `resolve()` is a second
    /// pointer cast — no re-validation, no allocation.
    ///
    /// ```ignore
    /// match ctx.accounts.oracle.resolve()? {
    ///     OraclePrice::Pyth(price) => { /* read Pyth fields */ }
    ///     OraclePrice::Switchboard(price) => { /* read Switchboard fields */ }
    /// }
    /// ```
    #[inline(always)]
    pub fn resolve(&self) -> Result<T::Resolved<'_>, ProgramError> {
        T::resolve(&self.view)
    }
}

/// Token interface program type — accepts either SPL Token or Token-2022.
///
/// Validates that the account is executable and its address matches one of
/// the two token program IDs. Provides the same CPI methods as [`TokenProgram`].
///
/// ```ignore
/// pub token_program: &'info TokenInterface,
/// ```
#[repr(transparent)]
pub struct TokenInterface {
    view: AccountView,
}

impl AsAccountView for TokenInterface {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl TokenInterface {
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        if !view.executable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if view.address() != &SPL_TOKEN_ID && view.address() != &TOKEN_2022_ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    /// # Safety (invalid_reference_casting)
    ///
    /// `Self` is `#[repr(transparent)]` over `AccountView`, which uses
    /// interior mutability through raw pointers to SVM account memory.
    /// The SVM runtime manages lamports and data as separate mutable
    /// regions behind raw pointers — `AccountView` never holds Rust
    /// references to these regions. The `&` → `&mut` cast therefore
    /// does not create aliased mutable references; all writes go
    /// through `AccountView`'s raw pointer methods. This pattern is
    /// standard in Solana frameworks (Pinocchio uses the same approach).
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        if !view.executable() {
            return Err(ProgramError::InvalidAccountData);
        }
        if view.address() != &SPL_TOKEN_ID && view.address() != &TOKEN_2022_ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

impl TokenCpi for TokenInterface {}
