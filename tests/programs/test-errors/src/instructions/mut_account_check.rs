use {crate::state::ErrorTestAccount, quasar_core::prelude::*};

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
