use quasar_core::prelude::*;

/// Tests: "Account 'destination' (index 1): must be writable"
#[derive(Accounts)]
pub struct HeaderDupMut<'info> {
    pub source: &'info Signer,
    /// CHECK: test-only — validates that dup mut accounts are parsed correctly.
    #[account(dup)]
    pub destination: &'info mut UncheckedAccount,
}

impl<'info> HeaderDupMut<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
