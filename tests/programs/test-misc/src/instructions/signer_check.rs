use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct SignerCheck<'info> {
    pub signer: &'info Signer,
}

impl<'info> SignerCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
