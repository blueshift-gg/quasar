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
    fn load(view: &AccountView) -> Result<Self, ProgramError> {
        Self::check(view)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked(view) as *const Self) })
    }

    #[inline(always)]
    fn load_mut(view: &mut AccountView) -> Result<Self, ProgramError> {
        Self::check(view)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked_mut(view) as *const Self) })
    }

    #[inline(always)]
    fn load_checked(view: &AccountView) -> Result<Self, ProgramError> {
        Self::check_checked(view)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked(view) as *const Self) })
    }

    #[inline(always)]
    fn load_mut_checked(view: &mut AccountView) -> Result<Self, ProgramError> {
        Self::check_checked(view)?;
        Ok(unsafe { core::ptr::read(Self::from_view_unchecked_mut(view) as *const Self) })
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
        &mut *(self as *mut Self as *mut AccountView)
    }
}
