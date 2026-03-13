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
