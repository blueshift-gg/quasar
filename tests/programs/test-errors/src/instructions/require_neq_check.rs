use quasar_core::prelude::*;

use crate::errors::TestError;

#[derive(Accounts)]
pub struct RequireNeqCheck<'info> {
    pub signer: &'info Signer,
}

impl<'info> RequireNeqCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64) -> Result<(), ProgramError> {
        require!(a != b, TestError::RequireEqFailed);
        Ok(())
    }
}
