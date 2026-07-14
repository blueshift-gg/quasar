use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub authority: Address,
    pub value: PodU64,
}

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub authority: Address,
    pub value: PodU64,
    pub extra: PodU32,
}

/// Hand-rolled (non-macro) migration source used to regression-test issue
/// #239 with source account buffers on both sides of [`PaddedTarget::SPACE`].
#[repr(transparent)]
pub struct PaddedSourceV1 {
    __view: AccountView,
}

unsafe impl StaticView for PaddedSourceV1 {}

impl AsAccountView for PaddedSourceV1 {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

impl Owner for PaddedSourceV1 {
    const OWNER: Address = crate::ID;
}

impl Discriminator for PaddedSourceV1 {
    const DISCRIMINATOR: &'static [u8] = &[9];
}

impl AccountLoad for PaddedSourceV1 {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: called only during account parsing, before any mutable
        // borrow of this view is taken.
        let data = unsafe { view.borrow_unchecked() };
        if !data.starts_with(<Self as Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Raw `#[repr(C)]` fixed portion written by [`PaddedTarget::migrate`].
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PaddedTargetData {
    pub authority: Address,
    pub value: PodU64,
}

/// Migration target whose `SPACE` reserves bytes beyond the discriminator
/// plus [`PaddedTargetData`] (a manual analogue of an account with padding
/// reserved for future fields). Regression coverage for issue #239: those
/// reserved bytes must come back zeroed, not carry over stale `From` data.
#[repr(transparent)]
pub struct PaddedTarget {
    __view: AccountView,
}

unsafe impl StaticView for PaddedTarget {}

impl AsAccountView for PaddedTarget {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

impl Owner for PaddedTarget {
    const OWNER: Address = crate::ID;
}

impl Discriminator for PaddedTarget {
    const DISCRIMINATOR: &'static [u8] = &[3];
}

impl Space for PaddedTarget {
    const SPACE: usize =
        1 + core::mem::size_of::<PaddedTargetData>() + PaddedTarget::RESERVED_PADDING;
}

impl PaddedTarget {
    pub const RESERVED_PADDING: usize = 20;
}

impl Deref for PaddedTarget {
    type Target = PaddedTargetData;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        // SAFETY: `AccountLoad::check` validates the discriminator and length
        // before this wrapper is constructed.
        unsafe { &*(self.__view.data_ptr().add(1) as *const PaddedTargetData) }
    }
}

impl DerefMut for PaddedTarget {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: same as `Deref::deref`, with exclusive access to the view.
        unsafe { &mut *(self.__view.data_mut_ptr().add(1) as *mut PaddedTargetData) }
    }
}

impl AccountLoad for PaddedTarget {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: called only during account parsing/revalidation, never
        // while another borrow of this view is live.
        let data = unsafe { view.borrow_unchecked() };
        if !data.starts_with(<Self as Discriminator>::DISCRIMINATOR) {
            return Err(ProgramError::InvalidAccountData);
        }
        if data.len() < <Self as Space>::SPACE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }
}
