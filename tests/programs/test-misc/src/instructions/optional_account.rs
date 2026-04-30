use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionalAccount {
    pub required: Account<SimpleAccount>,
    pub optional: Option<Account<SimpleAccount>>,
}

impl OptionalAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
