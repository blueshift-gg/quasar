use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'account' (index 0): must be writable with no duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupMut {
    #[account(mut)]
    pub account: UncheckedAccount,
}

impl HeaderNoDupMut {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
