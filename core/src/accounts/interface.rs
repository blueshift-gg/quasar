use crate::prelude::*;

/// A wrapper for program interface accounts that accept multiple program IDs.
///
/// Similar to `Program<T>` but validates against multiple allowed addresses
/// via the [`ProgramInterface`] trait.
///
/// # Example
/// ```ignore
/// pub struct TokenProgramInterface;
/// impl ProgramInterface for TokenProgramInterface {
///     fn matches(address: &Address) -> bool {
///         *address == TOKEN_PROGRAM_ID || *address == TOKEN_2022_PROGRAM_ID
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Transfer<'info> {
///     pub token_program: &'info Interface<TokenProgramInterface>,
/// }
/// ```
#[repr(transparent)]
pub struct Interface<T: ProgramInterface> {
    view: AccountView,
    _marker: core::marker::PhantomData<T>,
}

impl<T: ProgramInterface> AsAccountView for Interface<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: ProgramInterface> Interface<T> {
    /// # Safety
    /// Caller must ensure executable flag and address match.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }
}
