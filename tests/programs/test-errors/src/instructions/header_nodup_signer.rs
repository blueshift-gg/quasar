use quasar_core::prelude::*;

/// Tests: "Account 'account' (index 0): must be signer with no duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupSigner<'info> {
    pub account: &'info Signer,
}

impl<'info> HeaderNoDupSigner<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
