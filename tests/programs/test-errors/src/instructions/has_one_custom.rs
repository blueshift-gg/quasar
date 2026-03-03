use quasar_core::prelude::*;

use crate::errors::TestError;
use crate::state::ErrorTestAccount;

#[derive(Accounts)]
pub struct HasOneCustom<'info> {
    pub authority: &'info Signer,
    #[account(has_one = authority @ TestError::Hello)]
    pub account: &'info Account<ErrorTestAccount>,
}

impl<'info> HasOneCustom<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
