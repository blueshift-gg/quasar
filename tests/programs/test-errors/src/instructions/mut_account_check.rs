use {crate::state::ErrorTestAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct MutAccountCheck {
    #[account(mut)]
    pub account: Account<ErrorTestAccount>,
}

impl MutAccountCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
