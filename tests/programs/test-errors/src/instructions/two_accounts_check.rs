use {crate::state::ErrorTestAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct TwoAccountsCheck {
    pub first: Account<ErrorTestAccount>,
    pub second: Account<ErrorTestAccount>,
}

impl TwoAccountsCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
