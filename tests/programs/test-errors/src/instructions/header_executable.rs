use quasar_core::prelude::*;

/// Tests: "Account 'program' (index 0): must be executable program with no duplicates"
#[derive(Accounts)]
pub struct HeaderExecutable<'info> {
    pub program: &'info Program<System>,
}

impl<'info> HeaderExecutable<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
