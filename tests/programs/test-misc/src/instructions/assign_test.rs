use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct AssignTest {
    #[account(mut)]
    pub account: Signer,
    pub system_program: Program<SystemProgram>,
}

impl AssignTest {
    #[inline(always)]
    pub fn handler(&self, owner: Address) -> Result<(), ProgramError> {
        self.system_program.assign(&self.account, &owner).invoke()
    }
}
