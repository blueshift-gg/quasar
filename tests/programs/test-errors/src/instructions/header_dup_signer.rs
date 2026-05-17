use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'authority' (index 1): must be signer"
#[derive(Accounts)]
pub struct HeaderDupSigner {
    #[account(mut)]
    pub payer: Signer,
    /// Test-only duplicate signer account.
    #[account(dup)]
    pub authority: Signer,
}

impl HeaderDupSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
