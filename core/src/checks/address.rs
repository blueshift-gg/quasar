use crate::prelude::*;

pub trait Address: crate::traits::Id {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if view.address() != &Self::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(())
    }
}
