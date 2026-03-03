use quasar_core::prelude::*;

use crate::errors::TestError;

#[derive(Accounts)]
pub struct CustomError<'info> {
    pub signer: &'info Signer,
}

impl<'info> CustomError<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Err(TestError::Hello.into())
    }
}
