use {
    crate::{
        helpers::constants::{SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
    },
    core::marker::PhantomData,
    quasar_core::prelude::*,
};

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
    /// Construct an interface account reference from an `AccountView`,
    /// validating that the owner is SPL Token or Token-2022.
    ///
    /// # Errors
    ///
    /// Returns `IllegalOwner` if the owner is neither SPL Token nor
    /// Token-2022, or any error from `T::check`.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        let owner = view.owner();
        if quasar_core::utils::hint::unlikely(
            !quasar_core::keys_eq(owner, &SPL_TOKEN_ID)
                && !quasar_core::keys_eq(owner, &TOKEN_2022_ID),
        ) {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
        // SAFETY: `InterfaceAccount<T>` is `#[repr(transparent)]` over
        // `AccountView` — the pointer cast is layout-compatible. Owner
        // and data-length checks ran above.
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    /// Construct a mutable interface account reference from an
    /// `AccountView`, validating owner and writability.
    ///
    /// # Errors
    ///
    /// Returns `Immutable` if the account is not writable, `IllegalOwner`
    /// if the owner is neither SPL Token nor Token-2022, or any error
    /// from `T::check`.
    #[inline(always)]
    pub fn from_account_view_mut(view: &mut AccountView) -> Result<&mut Self, ProgramError> {
        if quasar_core::utils::hint::unlikely(!view.is_writable()) {
            return Err(ProgramError::Immutable);
        }
        let owner = view.owner();
        if quasar_core::utils::hint::unlikely(
            !quasar_core::keys_eq(owner, &SPL_TOKEN_ID)
                && !quasar_core::keys_eq(owner, &TOKEN_2022_ID),
        ) {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
        // SAFETY: Same as `from_account_view` — `#[repr(transparent)]`
        // guarantees layout compatibility. Writability checked above.
        Ok(unsafe { &mut *(view as *mut AccountView as *mut Self) })
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
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
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
        T::deref_from_mut(&mut self.view)
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
