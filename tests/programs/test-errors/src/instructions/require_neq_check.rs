use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct RequireNeqCheck {
    pub signer: Signer,
}

impl RequireNeqCheck {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64) -> Result<(), ProgramError> {
        require!(a != b, TestError::RequireEqFailed);
        Ok(())
    }
}
