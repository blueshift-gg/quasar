use {crate::errors::TestError, quasar_core::prelude::*};

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
