use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub user: &'info mut Signer,
    #[account(mut, seeds = [b"vault", user], bump)]
    pub vault: &'info mut UncheckedAccount,
    pub system_program: &'info SystemProgram,
}

impl<'info> Deposit<'info> {
    #[inline(always)]
    pub fn deposit(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(self.user, self.vault, amount)
            .invoke()
    }
}
