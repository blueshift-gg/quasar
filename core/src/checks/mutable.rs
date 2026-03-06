use crate::prelude::*;

/// Validates that an account was passed as writable in the transaction.
pub trait Mutable {
    /// Returns `Err(Immutable)` if the account is not writable.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(())
    }
}
