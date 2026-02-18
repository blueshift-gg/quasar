use crate::prelude::*;

pub trait Signer {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !view.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}
