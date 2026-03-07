use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct SignerMutCheck<'info> {
    pub signer: &'info mut Signer,
}

impl<'info> SignerMutCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
