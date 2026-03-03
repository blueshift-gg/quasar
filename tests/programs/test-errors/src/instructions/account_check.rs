use quasar_core::prelude::*;

use crate::state::ErrorTestAccount;

#[derive(Accounts)]
pub struct AccountCheckIx<'info> {
    pub account: &'info Account<ErrorTestAccount>,
}

impl<'info> AccountCheckIx<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
