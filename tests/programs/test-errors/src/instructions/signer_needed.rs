use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct SignerNeeded<'info> {
    pub signer: &'info Signer,
}

impl<'info> SignerNeeded<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
