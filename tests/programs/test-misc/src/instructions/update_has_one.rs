use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct UpdateHasOne {
    pub authority: Signer,
    #[account(has_one(authority), address = SimpleAccount::seeds(authority.address()))]
    pub account: Account<SimpleAccount>,
}

impl UpdateHasOne {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
