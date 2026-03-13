use {
    crate::{errors::TestError, state::SimpleAccount},
    quasar_core::prelude::*,
};

#[derive(Accounts)]
pub struct ConstraintCustomError<'info> {
    #[account(constraint = account.value > 0 @ TestError::CustomConstraint)]
    pub account: &'info Account<SimpleAccount>,
}

impl<'info> ConstraintCustomError<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
