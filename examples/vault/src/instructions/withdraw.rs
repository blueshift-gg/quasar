use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub user: &'info mut Signer,
    #[account(mut, seeds = [b"vault", user], bump)]
    pub vault: &'info mut UncheckedAccount,
    pub system_program: &'info SystemProgram,
}

impl<'info> Withdraw<'info> {
    #[inline(always)]
    pub fn withdraw(&self, amount: u64, bumps: &WithdrawBumps) -> Result<(), ProgramError> {
        let seeds = bumps.vault_seeds();
        self.system_program
            .transfer(self.vault, self.user, amount)
            .invoke_signed(&seeds)
    }
}
