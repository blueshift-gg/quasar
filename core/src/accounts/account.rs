use crate::cpi::system::SYSTEM_PROGRAM_ID;
use crate::prelude::*;
use crate::sysvars::Sysvar;
use core::marker::PhantomData;

#[repr(transparent)]
pub struct Account<T: Owner> {
    view: AccountView,
    _marker: PhantomData<T>,
}

impl<T: Owner> AsAccountView for Account<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Owner> Account<T> {
    #[inline(always)]
    pub fn owner(&self) -> &'static Address {
        &T::OWNER
    }
}

impl<T: Owner + AccountCheck> Account<T> {
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        if !view.owned_by(&T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
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
        if !view.owned_by(&T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        T::check(view)?;
        Ok(unsafe { &mut *(view as *const AccountView as *mut Self) })
    }
}

impl<T: QuasarAccount + Owner> Account<T> {
    #[inline(always)]
    pub fn get(&self) -> Result<T, ProgramError> {
        let data = self.view.try_borrow()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() || &data[..disc.len()] != disc {
            return Err(ProgramError::InvalidAccountData);
        }
        T::deserialize(&data[disc.len()..])
    }

    #[inline(always)]
    pub fn set(&mut self, value: &T) -> Result<(), ProgramError> {
        let mut data = self.view.try_borrow_mut()?;
        let disc = T::DISCRIMINATOR;
        value.serialize(&mut data[disc.len()..])
    }

    #[inline(always)]
    pub fn close(&self, destination: &AccountView) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        destination.set_lamports(destination.lamports() + view.lamports());
        view.set_lamports(0);
        unsafe { view.assign(&SYSTEM_PROGRAM_ID) };
        view.resize(0)?;
        Ok(())
    }

    #[inline(always)]
    pub fn realloc(
        &self,
        new_space: usize,
        payer: &AccountView,
        rent: Option<&crate::accounts::Rent>,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();

        let rent_exempt_lamports = match rent {
            Some(rent_account) => rent_account.get()?.try_minimum_balance(new_space)?,
            None => crate::sysvars::rent::Rent::get()?.try_minimum_balance(new_space)?,
        };

        let current_lamports = view.lamports();

        if rent_exempt_lamports > current_lamports {
            crate::cpi::system::transfer(payer, view, rent_exempt_lamports - current_lamports)
                .invoke()?;
        } else if current_lamports > rent_exempt_lamports {
            let excess = current_lamports - rent_exempt_lamports;
            view.set_lamports(rent_exempt_lamports);
            payer.set_lamports(payer.lamports() + excess);
        }

        view.resize(new_space)?;
        Ok(())
    }
}

impl<T: ZeroCopyDeref> core::ops::Deref for Account<T> {
    type Target = T::Target;

    /// SAFETY: Bounds validated by `AccountCheck::check` during `from_account_view`.
    /// ZC companion structs have compile-time `assert!(align_of::<Zc>() == 1)`,
    /// so the cast from account data pointer (align 1) is always well-aligned.
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.to_account_view().data_ptr().add(T::DATA_OFFSET) as *const T::Target) }
    }
}

impl<T: ZeroCopyDeref> core::ops::DerefMut for Account<T> {
    /// SAFETY: Same as Deref — bounds checked upstream, alignment 1 guaranteed.
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.to_account_view().data_ptr().add(T::DATA_OFFSET) as *mut T::Target) }
    }
}
