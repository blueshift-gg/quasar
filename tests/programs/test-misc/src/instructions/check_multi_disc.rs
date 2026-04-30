use {crate::state::MultiDiscAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CheckMultiDisc {
    pub account: Account<MultiDiscAccount>,
}

impl CheckMultiDisc {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
