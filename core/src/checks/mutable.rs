use crate::prelude::*;

pub trait Mutable {
    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if !view.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(())
    }
}
