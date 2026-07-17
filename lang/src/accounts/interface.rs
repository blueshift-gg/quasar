//! Program-interface account wrapper.
//!
//! `Interface<T>` is `#[repr(transparent)]` over `AccountView`. Parsing checks
//! the executable flag (`IS_EXECUTABLE`); `check` then rejects any address
//! `T::matches` does not accept. The later pointer-cast construction trusts
//! those two invariants (see the module header).

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

// SAFETY: `Interface<T>` is `#[repr(transparent)]` over `AccountView` plus
// `PhantomData<T>`, so the pointer cast preserves layout.
unsafe impl<T: ProgramInterface> crate::traits::StaticView for Interface<T> {}

impl<T: ProgramInterface> crate::account_load::AccountLoad for Interface<T> {
    const IS_EXECUTABLE: bool = true;

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if crate::utils::hint::unlikely(!T::matches(view.address())) {
            // Alloc-free so the diagnostic works under `no_alloc!`: log a static
            // message plus the actual (non-matching) address bytes.
            #[cfg(feature = "debug")]
            {
                crate::prelude::log(
                    "Program interface mismatch: address does not match any allowed program:",
                );
                crate::log::log_data(&[view.address().as_array()]);
            }
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
        // SAFETY: transparent layout (see the `StaticView` impl); the caller
        // upholds the executable/address invariants.
        unsafe { &*(view as *const AccountView as *const Self) }
    }
}
