use {
    crate::{errors::TestError, state::SimpleAccount},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct ConstraintCustomError {
    #[account(constraints(account.value > 0) @ TestError::CustomConstraint)]
    pub account: Account<SimpleAccount>,
}

impl ConstraintCustomError {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
