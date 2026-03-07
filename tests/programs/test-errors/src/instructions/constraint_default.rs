use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct ConstraintDefault<'info> {
    #[account(constraint = false)]
    pub target: &'info SystemAccount,
}

impl<'info> ConstraintDefault<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
