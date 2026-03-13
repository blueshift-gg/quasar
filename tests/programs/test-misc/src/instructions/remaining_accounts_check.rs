use quasar_core::{prelude::*, remaining::RemainingAccounts};

#[derive(Accounts)]
pub struct RemainingAccountsCheck<'info> {
    pub authority: &'info Signer,
}

impl<'info> RemainingAccountsCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, remaining: RemainingAccounts) -> Result<(), ProgramError> {
        for account in remaining.iter() {
            let _ = account?;
        }
        Ok(())
    }
}
