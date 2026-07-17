//! Address check: an account's key must equal the type's program address.
//!
//! Returns `Err(IncorrectProgramId)` when `view.address()` does not match
//! `Id::ID`. Implemented for any `Id` type and composed into a `check(view)`
//! by the `define_account!` check-list.

use crate::{prelude::*, utils::hint::unlikely};

/// Validates that an account's address matches the expected [`Id::ID`].
pub trait Address: crate::traits::Id {
    /// Returns `Err(IncorrectProgramId)` if `view.address() != Self::ID`.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if unlikely(!crate::keys_eq(view.address(), &Self::ID)) {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(())
    }
}
