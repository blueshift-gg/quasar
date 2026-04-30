use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionalHasOne {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub account: Option<Account<SimpleAccount>>,
}

impl OptionalHasOne {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
