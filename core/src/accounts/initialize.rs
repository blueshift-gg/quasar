use crate::prelude::*;
use core::marker::PhantomData;

#[repr(transparent)]
pub struct Initialize<T: Discriminator> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: Discriminator> AsAccountView for Initialize<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Discriminator> Initialize<T> {
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        Ok(unsafe { &*(view as *const AccountView as *const Self) })
    }

    /// # Safety (invalid_reference_casting)
    ///
    /// `Self` is `#[repr(transparent)]` over `AccountView`, which uses interior
    /// mutability through raw pointers to SVM account memory. The `&` → `&mut`
    /// cast does not create aliased mutable references to backing memory — all
    /// writes go through `AccountView`'s raw pointer methods. This pattern is
    /// standard in Solana frameworks (Pinocchio uses the same approach).
    #[inline(always)]
    #[allow(invalid_reference_casting, clippy::mut_from_ref)]
    pub fn from_account_view_mut(view: &AccountView) -> Result<&mut Self, ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}
