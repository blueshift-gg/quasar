use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct RequireEqCheck {
    pub signer: Signer,
}

impl RequireEqCheck {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64) -> Result<(), ProgramError> {
        require_eq!(a, b, TestError::RequireEqFailed);
        Ok(())
    }
}
