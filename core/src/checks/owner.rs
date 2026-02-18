use crate::prelude::*;

pub trait Owner: crate::traits::Owner {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !view.owned_by(&Self::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}
