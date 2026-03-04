use quasar_core::prelude::*;

use crate::state::DynamicAccount;

#[derive(Accounts)]
pub struct DynamicAccountCheck<'info> {
    pub account: &'info Account<DynamicAccount<'info>>,
}

impl<'info> DynamicAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
