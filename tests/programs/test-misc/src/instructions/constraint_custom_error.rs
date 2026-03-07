use quasar_core::prelude::*;

use crate::errors::TestError;
use crate::state::SimpleAccount;

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
