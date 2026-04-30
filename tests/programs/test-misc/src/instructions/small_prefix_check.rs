use {crate::state::SmallPrefixAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct SmallPrefixCheck {
    pub account: Account<SmallPrefixAccount>,
}

impl SmallPrefixCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
