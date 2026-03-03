use quasar_core::prelude::*;

use crate::state::MultiDiscAccount;

#[derive(Accounts)]
pub struct CheckMultiDisc<'info> {
    pub account: &'info Account<MultiDiscAccount>,
}

impl<'info> CheckMultiDisc<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
