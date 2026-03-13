use {crate::state::MixedAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct MixedAccountCheck<'info> {
    pub account: Account<MixedAccount<'info>>,
}

impl<'info> MixedAccountCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
