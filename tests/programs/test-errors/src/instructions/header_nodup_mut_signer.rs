use quasar_core::prelude::*;

/// Tests: "Account 'account' (index 0): must be writable signer with no duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupMutSigner<'info> {
    pub account: &'info mut Signer,
}

impl<'info> HeaderNoDupMutSigner<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
