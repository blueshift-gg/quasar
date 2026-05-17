use crate::prelude::*;

/// Program account wrapper.
///
/// Generated parsing enforces the executable flag via `IS_EXECUTABLE`; this
/// wrapper validates the program address.
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

impl<T: crate::traits::Id> crate::traits::Id for Program<T> {
    const ID: Address = T::ID;
}

impl<T: crate::traits::Id> crate::account_load::AccountLoad for Program<T> {
    const IS_EXECUTABLE: bool = true;

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!crate::keys_eq(view.address(), &T::ID)) {
            #[cfg(feature = "debug")]
            crate::prelude::log(&::alloc::format!(
                "Incorrect program ID: expected {}, got {}",
                T::ID,
                view.address()
            ));
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(())
    }
}

impl<T: crate::traits::Id> Program<T> {
    /// # Safety
    /// Caller must ensure the executable flag and program address are valid.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        // SAFETY: `Program<T>` is `repr(transparent)` over `AccountView` plus
        // `PhantomData<T>`, so the reference cast preserves layout. The caller
        // upholds the executable/address invariants.
        unsafe { &*(view as *const AccountView as *const Self) }
    }

    /// Emit an event via self-CPI.
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
