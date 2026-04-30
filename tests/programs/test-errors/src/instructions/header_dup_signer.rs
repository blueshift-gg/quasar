use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'authority' (index 1): must be signer"
#[derive(Accounts)]
pub struct HeaderDupSigner {
    #[account(mut)]
    pub payer: Signer,
    /// CHECK: test-only — validates that dup signer accounts are parsed
    /// correctly.
    #[account(dup)]
    pub authority: Signer,
}

impl HeaderDupSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
