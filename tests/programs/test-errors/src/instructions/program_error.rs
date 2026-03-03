use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct ProgramErrorIx<'info> {
    pub signer: &'info Signer,
}

impl<'info> ProgramErrorIx<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Err(ProgramError::InvalidAccountData)
    }
}
