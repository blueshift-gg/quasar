use core::marker::PhantomData;

use quasar_core::prelude::*;

use crate::helpers::constants::{SPL_TOKEN_ID, TOKEN_2022_ID};
use crate::instructions::TokenCpi;

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

    /// # Safety
    /// Caller must ensure owner and discriminator are valid.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// # Safety
    /// Caller must ensure owner, discriminator, and writability.
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub unsafe fn from_account_view_unchecked_mut(view: &AccountView) -> &mut Self {
        &mut *(view as *const AccountView as *mut Self)
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

/// Marker type for the token program interface (SPL Token or Token-2022).
///
/// Use with the `Interface<T>` wrapper:
/// ```ignore
/// pub token_program: &'info Interface<TokenInterface>,
/// ```
pub struct TokenInterface;

impl ProgramInterface for TokenInterface {
    #[inline(always)]
    fn matches(address: &Address) -> bool {
        quasar_core::keys_eq(address, &SPL_TOKEN_ID)
            || quasar_core::keys_eq(address, &TOKEN_2022_ID)
    }
}

impl TokenCpi for quasar_core::accounts::Interface<TokenInterface> {}
