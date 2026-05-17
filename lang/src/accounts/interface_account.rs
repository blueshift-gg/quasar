use {crate::prelude::*, core::marker::PhantomData};

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

impl<T: crate::account_layout::AccountLayout> crate::account_layout::AccountLayout
    for InterfaceAccount<T>
{
    type Schema = T::Schema;
    type Target = T::Target;
    const DATA_OFFSET: usize = T::DATA_OFFSET;
}

impl<T: Owners + crate::account_load::AccountLoad> InterfaceAccount<T> {
    /// Validate owner + data check, then pointer-cast.
    #[inline(always)]
    pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
        <T as Owners>::check_owner(view)?;
        T::check(view)?;
        // SAFETY: Owner and account data were validated above.
        Ok(unsafe { Self::from_account_view_unchecked(view) })
    }

    #[inline(always)]
    pub fn from_account_view_mut(view: &mut AccountView) -> Result<&mut Self, ProgramError> {
        if crate::utils::hint::unlikely(!view.is_writable()) {
            return Err(ProgramError::Immutable);
        }
        <T as Owners>::check_owner(view)?;
        T::check(view)?;
        // SAFETY: Writability, owner, and account data were validated above.
        Ok(unsafe { Self::from_account_view_unchecked_mut(view) })
    }

    /// # Safety
    /// Caller must ensure the owner and account data satisfy `T`'s validation
    /// rules.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        // SAFETY: `InterfaceAccount<T>` is `repr(transparent)` over
        // `AccountView` plus `PhantomData<T>`, so the reference cast preserves
        // layout. The caller upholds the owner/data invariants.
        unsafe { &*(view as *const AccountView as *const Self) }
    }

    /// # Safety
    /// Same as [`from_account_view_unchecked`](Self::from_account_view_unchecked),
    /// plus the account must be writable.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked_mut(view: &mut AccountView) -> &mut Self {
        // SAFETY: Same layout argument as the immutable cast; the caller also
        // guarantees writable access.
        unsafe { &mut *(view as *mut AccountView as *mut Self) }
    }
}

impl<T: Owners + crate::account_load::AccountLoad> crate::account_load::AccountLoad
    for InterfaceAccount<T>
{
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        <T as Owners>::check_owner(view)?;
        T::check(view)
    }

    #[inline(always)]
    fn check_checked(view: &AccountView) -> Result<(), ProgramError> {
        <T as Owners>::check_owner(view)?;
        T::check_checked(view)
    }
}

impl<T: ZeroCopyDeref> core::ops::Deref for InterfaceAccount<T> {
    type Target = T::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        // SAFETY: `InterfaceAccount<T>` can only be constructed safely after
        // owner and account-data validation; unsafe constructors require the
        // same invariant from the caller.
        unsafe { T::deref_from(&self.view) }
    }
}

impl<T: ZeroCopyDeref> core::ops::DerefMut for InterfaceAccount<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Same validation invariant as `deref`; `&mut self` also
        // provides exclusive access to the underlying account view.
        unsafe { T::deref_from_mut(&mut self.view) }
    }
}

impl<T: crate::account_init::AccountInit> crate::account_init::AccountInit for InterfaceAccount<T> {
    type InitParams<'a> = T::InitParams<'a>;
    const DEFAULT_INIT_PARAMS_VALID: bool = T::DEFAULT_INIT_PARAMS_VALID;

    #[inline(always)]
    fn init<'a, R: crate::ops::RentAccess>(
        ctx: crate::account_init::InitCtx<'a, R>,
        params: &Self::InitParams<'a>,
    ) -> solana_program_error::ProgramResult {
        T::init(ctx, params)
    }
}

impl<T: crate::traits::Space> crate::traits::Space for InterfaceAccount<T> {
    const SPACE: usize = T::SPACE;
}

impl<T: crate::ops::SupportsRealloc> crate::ops::SupportsRealloc for InterfaceAccount<T> {}
