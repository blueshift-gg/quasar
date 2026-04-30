use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ExplicitError {
    pub signer: Signer,
}

impl ExplicitError {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Err(TestError::ExplicitNum.into())
    }
}
