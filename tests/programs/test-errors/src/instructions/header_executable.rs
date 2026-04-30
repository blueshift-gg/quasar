use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'program' (index 0): must be executable program with no
/// duplicates"
#[derive(Accounts)]
pub struct HeaderExecutable {
    pub program: Program<SystemProgram>,
}

impl HeaderExecutable {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
