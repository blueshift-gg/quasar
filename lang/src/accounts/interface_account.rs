use {crate::prelude::*, core::marker::PhantomData};

#[inline(always)]
fn check_owners(view: &AccountView, owners: &[Address]) -> Result<(), ProgramError> {
    let owner = view.owner();
    let mut i = 0;
    while i < owners.len() {
        if crate::keys_eq(owner, &owners[i]) {
            return Ok(());
        }
        i += 1;
    }
    Err(ProgramError::IllegalOwner)
}

/// Account wrapper accepting any owner in `T::owners()` (e.g. SPL Token +
/// Token-2022).
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

impl<T: Owners + AccountCheck> InterfaceAccount<T> {
    /// Validate owner + data check, then pointer-cast.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        check_owners(view, T::owners())?;
        T::check(view)?;
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }
    #[inline(always)]
    pub fn from_account_view_mut(view: &mut AccountView) -> Result<&mut Self, ProgramError> {
        if crate::utils::hint::unlikely(!view.is_writable()) {
            return Err(ProgramError::Immutable);
        }
        check_owners(view, T::owners())?;
        T::check(view)?;
        Ok(unsafe { &mut *(view as *mut AccountView as *mut Self) })
    }

    /// # Safety
    /// Caller must ensure valid owner and data length.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// # Safety
    /// Same as above, plus account must be writable.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        &mut *(view as *mut AccountView as *mut Self)
    }
}

impl<T: Owners + AccountCheck> crate::account_load::AccountLoad for InterfaceAccount<T> {
    type Params = <T as AccountCheck>::Params;

    #[inline(always)]
    fn check(view: &AccountView, _field_name: &str) -> Result<(), ProgramError> {
        check_owners(view, T::owners())?;
        T::check(view)
    }

    #[inline(always)]
    fn validate(&self, params: &Self::Params) -> Result<(), ProgramError> {
        <T as AccountCheck>::validate(&self.view, params)
    }
}

impl<T: ZeroCopyDeref> core::ops::Deref for InterfaceAccount<T> {
    type Target = T::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { T::deref_from(&self.view) }
    }
}

impl<T: ZeroCopyDeref> core::ops::DerefMut for InterfaceAccount<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { T::deref_from_mut(&mut self.view) }
    }
}
