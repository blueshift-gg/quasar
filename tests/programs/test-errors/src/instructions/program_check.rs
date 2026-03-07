use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct ProgramCheck<'info> {
    pub program: &'info Program<System>,
}

impl<'info> ProgramCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
