use {crate::errors::TestError, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct RequireEqCheck<'info> {
    pub signer: &'info Signer,
}

impl<'info> RequireEqCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64) -> Result<(), ProgramError> {
        require_eq!(a, b, TestError::RequireEqFailed);
        Ok(())
    }
}
