use core::marker::PhantomData;
use crate::sysvars::Sysvar;
use crate::prelude::*;

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

    #[inline(always)]
    #[allow(invalid_reference_casting)]
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
        if data.first() != Some(&T::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }
        T::deserialize(&data[1..])
    }

    #[inline(always)]
    pub fn set(&mut self, value: &T) -> Result<(), ProgramError> {
        let mut data = self.view.try_borrow_mut()?;
        value.serialize(&mut data[1..])
    }

    #[inline(always)]
    pub fn close(&self, destination: &AccountView) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        destination.set_lamports(destination.lamports() + view.lamports());
        view.set_lamports(0);
        unsafe { view.assign(&Address::new_from_array([0u8; 32])) };
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

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*T::deref_data(self.to_account_view().borrow_unchecked().as_ptr()) }
    }
}

impl<T: QuasarAccount + Owner> core::ops::DerefMut for Account<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.to_account_view().borrow_unchecked_mut().as_mut_ptr().add(1) as *mut T) }
    }
}
