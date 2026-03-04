use quasar_core::prelude::*;

use crate::state::MixedAccount;

#[derive(Accounts)]
pub struct MixedAccountCheck<'info> {
    pub account: &'info Account<MixedAccount<'info>>,
}

impl<'info> MixedAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
