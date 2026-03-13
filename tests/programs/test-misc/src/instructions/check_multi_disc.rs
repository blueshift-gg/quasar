use {crate::state::MultiDiscAccount, quasar_core::prelude::*};

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
