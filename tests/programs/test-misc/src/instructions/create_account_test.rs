use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct CreateAccountTest<'info> {
    pub payer: &'info mut Signer,
    pub new_account: &'info mut Signer,
    pub system_program: &'info SystemProgram,
}

impl<'info> CreateAccountTest<'info> {
    #[inline(always)]
    pub fn handler(&self, lamports: u64, space: u64, owner: Address) -> Result<(), ProgramError> {
        self.system_program
            .create_account(self.payer, self.new_account, lamports, space, &owner)
            .invoke()
    }
}
