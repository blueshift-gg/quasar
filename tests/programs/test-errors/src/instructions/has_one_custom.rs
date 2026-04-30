use {
    crate::{errors::TestError, state::ErrorTestAccount},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct HasOneCustom {
    pub authority: Signer,
    #[account(has_one(authority) @ TestError::Hello)]
    pub account: Account<ErrorTestAccount>,
}

impl HasOneCustom {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
