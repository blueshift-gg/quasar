use {crate::traits::AsAccountView, core::marker::PhantomData, solana_account_view::AccountView};

/// Sysvar account wrapper. Validates address = `T::ID`, derefs to `T`.
#[repr(transparent)]
pub struct Sysvar<T: crate::sysvars::Sysvar> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: crate::sysvars::Sysvar> Sysvar<T> {
    /// # Safety
    /// Caller must ensure `view.address() == T::ID`.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    #[inline(always)]
    pub fn get(&self) -> &T {
        unsafe { T::from_bytes_unchecked(self.view.borrow_unchecked()) }
    }
}

impl<T: crate::sysvars::Sysvar> crate::account_load::AccountLoad for Sysvar<T> {
    type Params = ();

    #[inline(always)]
    fn check(
        view: &AccountView,
        field_name: &str,
    ) -> Result<(), solana_program_error::ProgramError> {
        crate::validation::check_sysvar::<T>(view, field_name)
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
