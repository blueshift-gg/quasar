use quasar_core::prelude::*;

use crate::state::ErrorTestAccount;

#[derive(Accounts)]
pub struct TwoAccountsCheck<'info> {
    pub first: &'info Account<ErrorTestAccount>,
    pub second: &'info Account<ErrorTestAccount>,
}

impl<'info> TwoAccountsCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
