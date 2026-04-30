use {crate::state::DynamicAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicAccountCheck {
    pub account: Account<DynamicAccount>,
}

impl DynamicAccountCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
