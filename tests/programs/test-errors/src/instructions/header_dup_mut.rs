use quasar_core::prelude::*;

/// Tests: "Account 'destination' (index 1): must be writable"
#[derive(Accounts)]
#[account(dup)]
pub struct HeaderDupMut<'info> {
    pub source: &'info Signer,
    pub destination: &'info mut UncheckedAccount,
}

impl<'info> HeaderDupMut<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
