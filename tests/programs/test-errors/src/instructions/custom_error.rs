use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CustomError {
    pub signer: Signer,
}

impl CustomError {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Err(TestError::Hello.into())
    }
}
