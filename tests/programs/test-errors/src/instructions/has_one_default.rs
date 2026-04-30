use {crate::state::ErrorTestAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct HasOneDefault {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub account: Account<ErrorTestAccount>,
}

impl HasOneDefault {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
