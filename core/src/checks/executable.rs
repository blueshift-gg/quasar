use crate::{prelude::*, utils::hint::unlikely};

/// Validates that an account is marked as executable (i.e., a program account).
pub trait Executable {
    /// Returns `Err(InvalidAccountData)` if the account is not executable.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if unlikely(!view.executable()) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}
