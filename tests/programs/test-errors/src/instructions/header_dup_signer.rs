use quasar_core::prelude::*;

/// Tests: "Account 'authority' (index 1): must be signer"
#[derive(Accounts)]
pub struct HeaderDupSigner<'info> {
    pub payer: &'info mut Signer,
    /// CHECK: test-only — validates that dup signer accounts are parsed
    /// correctly.
    #[account(dup)]
    pub authority: &'info Signer,
}

impl<'info> HeaderDupSigner<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
