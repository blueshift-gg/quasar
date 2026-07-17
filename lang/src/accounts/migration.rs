//! The [`Migration`] wrapper for typed on-chain schema migration.
//!
//! `Migration<From, To>` is `#[repr(transparent)]` over [`AccountView`] and
//! is constructed only after the `From` account is validated, so its `Deref`
//! to `From::Target` re-relies on that construction invariant. `migrate`
//! rewrites the account to the `To` layout, revalidates it, and returns an
//! `&mut Account<To>`.

use {crate::prelude::*, core::marker::PhantomData};

/// Account wrapper for type-safe on-chain migration from `From` to `To`.
///
/// The type validates and dereferences as `From` during account parsing. The
/// handler owns the lifecycle transition by calling `.migrate(&payer, data)`,
/// which reallocates if needed, writes the `To` layout, validates it, and
/// returns `&mut Account<To>`.
#[repr(transparent)]
pub struct Migration<From, To> {
    __view: AccountView,
    _marker: PhantomData<(From, To)>,
}

impl<From, To> AsAccountView for Migration<From, To> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

// SAFETY: `Migration<From, To>` is `repr(transparent)` over `AccountView` plus
// `PhantomData<(From, To)>`.
unsafe impl<From, To> crate::traits::StaticView for Migration<From, To> {}

impl<From, To> crate::account_load::AccountLoad for Migration<From, To>
where
    From: CheckOwner + crate::account_load::AccountLoad,
    To: crate::traits::Space + crate::traits::Discriminator,
{
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        From::check_owner(view)?;
        From::check(view)
    }

    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        From::check_owner(view)?;
        From::check_checked(view)
    }
}

impl<From, To> core::ops::Deref for Migration<From, To>
where
    From: core::ops::Deref + crate::traits::Discriminator,
    From::Target: Sized,
{
    type Target = From::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        let disc_len = <From as crate::traits::Discriminator>::DISCRIMINATOR.len();
        // SAFETY: `AccountLoad::check` validated the source discriminator and
        // account length before this wrapper was constructed. `From::Target`
        // is a `#[repr(C)]` zero-copy companion with alignment 1, so the
        // offset read is validly typed.
        unsafe { &*(self.__view.data_ptr().add(disc_len) as *const From::Target) }
    }
}

impl<From, To> Migration<From, To>
where
    From: crate::traits::Discriminator + crate::traits::Owner,
    To: crate::account_load::AccountLoad
        + CheckOwner
        + core::ops::Deref
        + crate::traits::Owner
        + crate::traits::Space
        + crate::traits::Discriminator
        + crate::traits::StaticView,
    To::Target: Sized,
{
    const _OWNER_EQ: () = assert!(
        crate::keys_eq_const(
            &<From as crate::traits::Owner>::OWNER,
            &<To as crate::traits::Owner>::OWNER,
        ),
        "migration source and target must have the same Owner"
    );
    const _DISC_NEQ: () = {
        let src = <From as crate::traits::Discriminator>::DISCRIMINATOR;
        let tgt = <To as crate::traits::Discriminator>::DISCRIMINATOR;
        let min_len = if src.len() < tgt.len() {
            src.len()
        } else {
            tgt.len()
        };
        let mut i = 0;
        let mut prefix_match = true;
        while i < min_len {
            if src[i] != tgt[i] {
                prefix_match = false;
            }
            i += 1;
        }
        assert!(
            !prefix_match,
            "migration source and target discriminators must not be prefixes of each other"
        );
    };
    // The target moves by value through one call layer, so 512 bytes of
    // the frame are reserved for that layer's locals.
    const _STACK_BUDGET: () = assert!(
        core::mem::size_of::<To::Target>() < crate::__internal::SBF_STACK_FRAME - 512,
        "migration target type too large for the sBPF stack budget"
    );
    const _TARGET_FITS_SPACE: () = assert!(
        <To as crate::traits::Space>::SPACE
            >= <To as crate::traits::Discriminator>::DISCRIMINATOR.len()
                + core::mem::size_of::<To::Target>(),
        "migration target Space must cover discriminator plus target data"
    );

    #[inline(always)]
    fn assert_migration_contract() {
        #[allow(clippy::let_unit_value)]
        {
            let _ = Self::_OWNER_EQ;
            let _ = Self::_DISC_NEQ;
            let _ = Self::_STACK_BUDGET;
            let _ = Self::_TARGET_FITS_SPACE;
        }
    }

    #[inline(always)]
    fn check_source_ready(&self) -> Result<(), ProgramError> {
        // SAFETY: `Migration` is only constructed after `From` account
        // validation, so reading the account data for discriminator checks is
        // valid here.
        let data = unsafe { self.__view.borrow_unchecked() };
        if data.starts_with(<To as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        if !data.starts_with(<From as crate::traits::Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn write_target(&mut self, new_data: &To::Target) {
        let disc = <To as crate::traits::Discriminator>::DISCRIMINATOR;
        let written_len = disc.len() + core::mem::size_of::<To::Target>();
        let space = <To as crate::traits::Space>::SPACE;
        let data = self.__view.data_mut_ptr();
        // SAFETY: `migrate` reallocates to `To::SPACE` before calling this, and
        // `_TARGET_FITS_SPACE` proves `To::SPACE` covers the discriminator plus
        // `To::Target`, so `written_len <= space == self.__view.data_len()`.
        unsafe {
            core::ptr::copy_nonoverlapping(disc.as_ptr(), data, disc.len());
            core::ptr::copy_nonoverlapping(
                new_data as *const To::Target as *const u8,
                data.add(disc.len()),
                core::mem::size_of::<To::Target>(),
            );
            // `To::SPACE` may reserve bytes beyond the discriminator + target
            // (e.g. for future fields). Those bytes are never written above,
            // so without this they'd retain stale `From` data left over from
            // before the realloc when `To::SPACE <= From::SPACE`.
            if space > written_len {
                core::ptr::write_bytes(data.add(written_len), 0, space - written_len);
            }
        }
    }

    /// Migrate to the new schema and return the initialized target account.
    #[inline(always)]
    pub fn migrate(
        &mut self,
        payer: &impl AsAccountView,
        new_data: To::Target,
    ) -> Result<&mut Account<To>, ProgramError> {
        Self::assert_migration_contract();
        self.check_source_ready()?;
        crate::accounts::realloc_account(
            &mut self.__view,
            <To as crate::traits::Space>::SPACE,
            payer.to_account_view(),
            None,
        )?;
        self.write_target(&new_data);
        <Account<To> as crate::account_load::AccountLoad>::check(&self.__view)?;
        // SAFETY: The target bytes were written above and immediately
        // revalidated as `Account<To>`.
        Ok(unsafe { Account::<To>::from_account_view_unchecked_mut(&mut self.__view) })
    }
}
