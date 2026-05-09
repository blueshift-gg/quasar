use {crate::prelude::*, core::marker::PhantomData};

/// Account slot that must be initialized explicitly by the handler.
///
/// `Uninit<A>` is intentionally only a parse-time marker. The derive loads the
/// raw writable account slot, and `.init(...)` performs the lifecycle
/// transition into the target account type.
#[repr(transparent)]
pub struct Uninit<A> {
    __view: AccountView,
    _marker: PhantomData<A>,
}

impl<A> AsAccountView for Uninit<A> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

unsafe impl<A> crate::traits::StaticView for Uninit<A> {}

impl<A> crate::account_load::AccountLoad for Uninit<A> {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!crate::is_system_program(view.owner())) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        Ok(())
    }
}

/// Runtime deferred initialization strategy for `Uninit<A>`.
///
/// The parameter type owns how an uninitialized slot becomes `A`. Plain
/// program accounts use their generated account data struct; protocol accounts
/// can implement this for protocol-specific init parameter structs.
pub trait DeferredInit<A> {
    fn init_uninit<'a>(
        self,
        target: &'a mut AccountView,
        payer: &AccountView,
        signers: &[crate::cpi::Signer<'_, '_>],
    ) -> Result<&'a mut A, ProgramError>;
}

impl<A> Uninit<A> {
    #[inline(always)]
    fn view_mut(&mut self) -> &mut AccountView {
        &mut self.__view
    }

    #[inline(always)]
    pub fn init<P>(&mut self, payer: &impl AsAccountView, params: P) -> Result<&mut A, ProgramError>
    where
        P: DeferredInit<A>,
    {
        params.init_uninit(self.view_mut(), payer.to_account_view(), &[])
    }

    #[inline(always)]
    pub fn init_signed<P>(
        &mut self,
        payer: &impl AsAccountView,
        params: P,
        signers: &[crate::cpi::Signer<'_, '_>],
    ) -> Result<&mut A, ProgramError>
    where
        P: DeferredInit<A>,
    {
        params.init_uninit(self.view_mut(), payer.to_account_view(), signers)
    }
}

impl<T> DeferredInit<Account<T>> for T::Target
where
    T: crate::account_load::AccountLoad
        + CheckOwner
        + core::ops::Deref
        + crate::traits::Discriminator
        + crate::traits::Owner
        + crate::traits::Space
        + crate::traits::StaticView,
    T::Target: Sized,
{
    #[inline(always)]
    fn init_uninit<'a>(
        self,
        target: &'a mut AccountView,
        payer: &AccountView,
        signers: &[crate::cpi::Signer<'_, '_>],
    ) -> Result<&'a mut Account<T>, ProgramError> {
        if crate::utils::hint::unlikely(!crate::is_system_program(target.owner())) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        let rent = <crate::sysvars::rent::Rent as crate::sysvars::Sysvar>::get()?;
        crate::account_init::init_account(
            payer,
            target,
            <T as crate::traits::Space>::SPACE as u64,
            &<T as crate::traits::Owner>::OWNER,
            signers,
            &rent,
            <T as crate::traits::Discriminator>::DISCRIMINATOR,
        )?;

        let disc_len = <T as crate::traits::Discriminator>::DISCRIMINATOR.len();
        unsafe {
            core::ptr::copy_nonoverlapping(
                &self as *const T::Target as *const u8,
                target.data_mut_ptr().add(disc_len),
                core::mem::size_of::<T::Target>(),
            );
        }

        <Account<T> as crate::account_load::AccountLoad>::check(target)?;
        Ok(unsafe { Account::<T>::from_account_view_unchecked_mut(target) })
    }
}
