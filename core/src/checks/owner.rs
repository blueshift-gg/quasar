use crate::{prelude::*, utils::hint::unlikely};

/// Validates that an account is owned by the expected program
/// ([`Owner::OWNER`](crate::traits::Owner::OWNER)).
pub trait Owner: crate::traits::Owner {
    /// Returns `Err(IllegalOwner)` if the account's owner does not match
    /// `Self::OWNER`.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if unlikely(!crate::keys_eq(view.owner(), &Self::OWNER)) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}
