use core::marker::PhantomData;
use crate::prelude::*;

#[repr(transparent)]
pub struct Initialize<T: QuasarAccount> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: QuasarAccount> AsAccountView for Initialize<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: QuasarAccount> Initialize<T> {

    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    #[inline(always)]
    #[allow(invalid_reference_casting)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}
