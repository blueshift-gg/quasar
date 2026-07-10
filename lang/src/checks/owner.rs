use crate::prelude::*;

/// Validates that an account is owned by the expected program
/// ([`Owner::OWNER`](crate::traits::Owner::OWNER)).
///
/// This is a thin delegate to the single-source single-owner check on
/// [`CheckOwner`]; it exists so the
/// `define_account!` check-list mechanism can compose owner validation through
/// a `check(view)` method.
pub trait Owner: crate::traits::Owner + crate::traits::CheckOwner {
    /// Returns `Err(IllegalOwner)` if the account's owner does not match
    /// `Self::OWNER`.
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        <Self as crate::traits::CheckOwner>::check_owner(view)
    }
}
