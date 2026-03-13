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
    /// The underlying account view.
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
    /// # Safety
    /// Caller must ensure executable flag and address are valid.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// Emit an event via CPI to this program.
    ///
    /// This method is used by `emit_cpi!` macro for self-CPI event emission.
    #[inline(always)]
    pub fn emit_event<E, EA>(
        &self,
        event: &E,
        event_authority: &EA,
        bump: u8,
    ) -> Result<(), solana_program_error::ProgramError>
    where
        E: crate::traits::Event,
        EA: AsAccountView,
    {
        let program = self.to_account_view();
        let ea = event_authority.to_account_view();
        event.emit(|data| crate::event::emit_event_cpi(program, ea, data, bump))
    }
}
