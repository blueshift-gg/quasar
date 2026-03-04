use crate::prelude::*;
use core::marker::PhantomData;

#[repr(transparent)]
pub struct Initialize<T> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T> AsAccountView for Initialize<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T> Initialize<T> {
    /// Unchecked construction for optimized parsing where the writable flag
    /// has been pre-validated via u32 header comparison during entrypoint
    /// deserialization.
    ///
    /// # Safety
    ///
    /// Caller must guarantee that the account's writable flag has been validated
    /// via u32 header check.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// Unchecked mutable construction for optimized parsing.
    ///
    /// # Safety (invalid_reference_casting + validation requirements)
    ///
    /// Caller must guarantee that the account's writable flag has been validated
    /// via u32 header check.
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub unsafe fn from_account_view_unchecked_mut(view: &AccountView) -> &mut Self {
        &mut *(view as *const AccountView as *mut Self)
    }
}
