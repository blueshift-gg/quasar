use {crate::traits::AsAccountView, core::marker::PhantomData, solana_account_view::AccountView};

/// Sysvar account wrapper.
///
/// Parsing validates the canonical sysvar address, then `get` views the account
/// data as `T`.
#[repr(transparent)]
pub struct Sysvar<T: crate::sysvars::Sysvar> {
    view: AccountView,
    _marker: PhantomData<T>,
}

// SAFETY: `Sysvar<T>` is `#[repr(transparent)]` over `AccountView` plus
// `PhantomData<T>`, so the pointer cast preserves layout.
unsafe impl<T: crate::sysvars::Sysvar> crate::traits::StaticView for Sysvar<T> {}

impl<T: crate::sysvars::Sysvar> Sysvar<T> {
    /// # Safety
    /// Caller must ensure `view.address() == T::ID` and that the account data
    /// contains a valid `T` sysvar layout.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        // SAFETY: `Sysvar<T>` is `repr(transparent)` over `AccountView` plus
        // `PhantomData<T>`, so the reference cast preserves layout. The caller
        // upholds the sysvar address/data invariant.
        unsafe { &*(view as *const AccountView as *const Self) }
    }

    /// Returns a zero-copy view of the sysvar account data.
    #[inline(always)]
    pub fn get(&self) -> &T {
        // SAFETY: Checked construction requires the canonical sysvar address;
        // unchecked construction makes the same data-layout guarantee explicit.
        unsafe { T::from_bytes_unchecked(self.view.borrow_unchecked()) }
    }
}

impl<T: crate::sysvars::Sysvar> crate::account_load::AccountLoad for Sysvar<T> {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), solana_program_error::ProgramError> {
        if crate::utils::hint::unlikely(!crate::keys_eq(view.address(), &T::ID)) {
            // Alloc-free so the diagnostic works under `no_alloc!`: log a static
            // message plus the expected/actual address bytes via `log_data`.
            #[cfg(feature = "debug")]
            {
                crate::prelude::log("Incorrect sysvar address (expected, actual):");
                crate::log::log_data(&[T::ID.as_array(), view.address().as_array()]);
            }
            return Err(solana_program_error::ProgramError::IncorrectProgramId);
        }
        Ok(())
    }
}

impl<T: crate::sysvars::Sysvar> AsAccountView for Sysvar<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: crate::sysvars::Sysvar> core::ops::Deref for Sysvar<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        self.get()
    }
}
