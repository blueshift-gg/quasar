use crate::prelude::*;

/// Program interface wrapper.
///
/// Generated parsing enforces the executable flag via `IS_EXECUTABLE`; this
/// wrapper validates the address against `ProgramInterface`.
#[repr(transparent)]
pub struct Interface<T: ProgramInterface> {
    view: AccountView,
    _marker: core::marker::PhantomData<T>,
}

impl<T: ProgramInterface> AsAccountView for Interface<T> {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

impl<T: ProgramInterface> crate::account_load::AccountLoad for Interface<T> {
    const IS_EXECUTABLE: bool = true;

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!T::matches(view.address())) {
            #[cfg(feature = "debug")]
            crate::prelude::log(&::alloc::format!(
                "Program interface mismatch: address {} does not match any allowed programs",
                view.address()
            ));
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(())
    }
}

impl<T: ProgramInterface> Interface<T> {
    /// # Safety
    /// Caller must ensure the executable flag is set and the address matches
    /// one of `T`'s allowed program IDs.
    #[inline(always)]
    pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
        // SAFETY: `Interface<T>` is `repr(transparent)` over `AccountView`
        // plus `PhantomData<T>`, so the reference cast preserves layout. The
        // caller upholds the executable/address invariants.
        unsafe { &*(view as *const AccountView as *const Self) }
    }
}
