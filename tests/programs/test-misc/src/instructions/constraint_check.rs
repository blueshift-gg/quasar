use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ConstraintCheck {
    #[account(constraints(account.value > 0))]
    pub account: Account<SimpleAccount>,
}

impl ConstraintCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
