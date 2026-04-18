use {
    crate::traits::AsAccountView, solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Unified validation, construction, and header flag consts for account wrapper
/// types.
///
/// All implementors must be `#[repr(transparent)]` over `AccountView`.
pub trait AccountLoad: AsAccountView + Sized {
    const IS_SIGNER: bool = false;
    const IS_EXECUTABLE: bool = false;

    /// Per-type config for namespaced constraints (e.g. `token::mint`).
    type Params: Default;

    /// Validate this account after header flag checks pass.
    ///
    /// Header flags (signer, writable, executable) are already validated by
    /// `parse_accounts` before this is called.
    fn check(view: &AccountView, field_name: &str) -> Result<(), ProgramError>;

    /// # Safety
    /// Caller must ensure the `AccountView` is valid for `#[repr(transparent)]`
    /// cast.
    #[inline(always)]
    unsafe fn from_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// # Safety
    /// Same as `from_view_unchecked`, plus the account must be writable.
    #[inline(always)]
    unsafe fn from_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
    }

    #[inline(always)]
    fn load(view: &AccountView, field_name: &str) -> Result<Self, ProgramError> {
        Self::check(view, field_name)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked(view) as *const Self) })
    }

    #[inline(always)]
    fn load_mut(view: &mut AccountView, field_name: &str) -> Result<Self, ProgramError> {
        Self::check(view, field_name)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked_mut(view) as *const Self) })
    }

    /// Validate namespaced constraints. No-op for types with `Params = ()`.
    #[inline(always)]
    fn validate(&self, _params: &Self::Params) -> Result<(), ProgramError> {
        Ok(())
    }
}
