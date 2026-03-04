use quasar_core::prelude::*;

use crate::state::SmallPrefixAccount;

#[derive(Accounts)]
pub struct SmallPrefixCheck<'info> {
    pub account: &'info Account<SmallPrefixAccount<'info>>,
}

impl<'info> SmallPrefixCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
