use crate::{prelude::*, utils::hint::unlikely};

/// Validates that an account signed the transaction.
pub trait Signer {
    /// Returns `Err(MissingRequiredSignature)` if the account is not a signer.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if unlikely(!view.is_signer()) {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}
