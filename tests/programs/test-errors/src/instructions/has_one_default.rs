use quasar_core::prelude::*;

use crate::state::ErrorTestAccount;

#[derive(Accounts)]
pub struct HasOneDefault<'info> {
    pub authority: &'info Signer,
    #[account(has_one = authority)]
    pub account: &'info Account<ErrorTestAccount>,
}

impl<'info> HasOneDefault<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
