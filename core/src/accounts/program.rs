use crate::prelude::*;

/// A wrapper for program accounts that validates executable flag and address.
///
/// Similar to `Account<T>` for data accounts and `Sysvar<T>` for sysvars, this
/// provides a generic way to handle any program account type.
///
/// # Example
/// ```ignore
/// #[derive(Accounts)]
/// pub struct MyAccounts<'info> {
///     pub system_program: &'info Program<system_program::SystemProgramId>,
///     pub token_program: &'info Program<token::TokenProgramId>,
/// }
/// ```
#[repr(transparent)]
pub struct Program<T: crate::traits::Id> {
    view: AccountView,
    _marker: core::marker::PhantomData<T>,
}

impl<T: crate::traits::Id> AsAccountView for Program<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

// Transparent Program trait forwarding - allows Program<T> to be used
// wherever Program trait is expected
impl<T: crate::traits::Id> crate::traits::Id for Program<T> {
    const ID: Address = T::ID;
}

impl<T: crate::traits::Id> Program<T> {
    /// Unchecked construction for optimized parsing where executable flag and address
    /// have been pre-validated during entrypoint deserialization.
    ///
    /// # Safety
    ///
    /// Caller must guarantee:
    /// 1. `view.executable()` is true (validated via header check)
    /// 2. `view.address() == T::ID` (validated explicitly)
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }
}
