use quasar_core::prelude::*;

use crate::state::ErrorTestAccount;

#[derive(Accounts)]
pub struct MutAccountCheck<'info> {
    #[account(mut)]
    pub account: &'info Account<ErrorTestAccount>,
}

impl<'info> MutAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
