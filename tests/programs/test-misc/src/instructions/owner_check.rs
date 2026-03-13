use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct OwnerCheck<'info> {
    pub account: &'info Account<SimpleAccount>,
}

impl<'info> OwnerCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
