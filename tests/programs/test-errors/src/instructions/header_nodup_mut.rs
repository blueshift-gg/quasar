use quasar_core::prelude::*;

/// Tests: "Account 'account' (index 0): must be writable with no duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupMut<'info> {
    pub account: &'info mut UncheckedAccount,
}

impl<'info> HeaderNoDupMut<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
