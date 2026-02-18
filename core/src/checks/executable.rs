use crate::prelude::*;

pub trait Executable {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !view.executable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}
