use {crate::errors::TestError, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ConstraintFail {
    #[account(constraints(false) @ TestError::ConstraintCustom)]
    pub target: SystemAccount,
}

impl ConstraintFail {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
