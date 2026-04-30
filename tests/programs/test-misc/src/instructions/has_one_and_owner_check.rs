use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct HasOneAndOwnerCheck {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub account: Account<SimpleAccount>,
}

impl HasOneAndOwnerCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
