use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct RequireFalse {
    pub signer: Signer,
}

impl RequireFalse {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        require!(false, TestError::RequireFailed);
        Ok(())
    }
}
