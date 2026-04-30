use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CreateAccountTest {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub new_account: Signer,
    pub system_program: Program<SystemProgram>,
}

impl CreateAccountTest {
    #[inline(always)]
    pub fn handler(&self, lamports: u64, space: u64, owner: Address) -> Result<(), ProgramError> {
        self.system_program
            .create_account(&self.payer, &self.new_account, lamports, space, &owner)
            .invoke()
    }
}
