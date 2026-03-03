use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct AssignTest<'info> {
    pub account: &'info mut Signer,
    pub system_program: &'info SystemProgram,
}

impl<'info> AssignTest<'info> {
    #[inline(always)]
    pub fn handler(&self, owner: Address) -> Result<(), ProgramError> {
        self.system_program.assign(self.account, &owner).invoke()
    }
}
