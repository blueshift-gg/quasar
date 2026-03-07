use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct SystemAccountCheck<'info> {
    pub account: &'info SystemAccount,
}

impl<'info> SystemAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
