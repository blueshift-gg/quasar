use {
    quasar_derive::Accounts,
    quasar_lang::{prelude::*, remaining::RemainingAccounts},
};

#[derive(Accounts)]
pub struct RemainingAccountsCheck {
    pub authority: Signer,
}

impl RemainingAccountsCheck {
    #[inline(always)]
    pub fn handler(&self, remaining: RemainingAccounts<'_>) -> Result<(), ProgramError> {
        for account in remaining.iter() {
            let _ = account?;
        }
        Ok(())
    }
}
