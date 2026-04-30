use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct UpdateAddress {
    #[account(address = crate::EXPECTED_ADDRESS)]
    pub target: Account<SimpleAccount>,
}

impl UpdateAddress {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
