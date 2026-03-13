use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct ConstraintCheck<'info> {
    #[account(constraint = account.value > 0)]
    pub account: &'info Account<SimpleAccount>,
}

impl<'info> ConstraintCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
