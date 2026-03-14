use {crate::state::DynamicAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct DynamicAccountCheck<'info> {
    pub account: Account<DynamicAccount<'info>>,
}

impl<'info> DynamicAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
