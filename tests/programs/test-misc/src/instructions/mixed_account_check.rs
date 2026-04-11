use {crate::state::MixedAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct MixedAccountCheck {
    pub account: Account<MixedAccount>,
}

impl MixedAccountCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
