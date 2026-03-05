use core::marker::PhantomData;
use solana_account_view::AccountView;

use crate::traits::AsAccountView;

/// Generic sysvar account wrapper. Validates the account address matches
/// `T::ID` on construction and provides zero-copy access to the sysvar data
/// via `Deref`.
///
/// Uses `borrow_unchecked` (no runtime borrow tracking) — sysvars are
/// always read-only, so there is no aliasing risk.
#[repr(transparent)]
pub struct Sysvar<T: crate::sysvars::Sysvar> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: crate::sysvars::Sysvar> Sysvar<T> {
    /// Unchecked construction for optimized parsing where the address has been
    /// pre-validated via explicit check during entrypoint deserialization.
    ///
    /// # Safety
    ///
    /// Caller must guarantee that `view.address() == T::ID`.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        &*(view as *const AccountView as *const Self)
    }

    /// Access the sysvar data without borrow tracking.
    ///
    /// SAFETY: The address was validated in `from_account_view`. Sysvars are
    /// read-only, so `borrow_unchecked` (no RefCell overhead) is sound —
    /// no mutable alias can exist.
    #[inline(always)]
    pub fn get(&self) -> &T {
        unsafe { T::from_bytes_unchecked(self.view.borrow_unchecked()) }
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
