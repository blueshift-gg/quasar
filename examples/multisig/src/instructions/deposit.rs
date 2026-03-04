use quasar_core::prelude::*;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub depositor: &'info mut Signer,
    pub config: &'info Account<MultisigConfig<'info>>,
    #[account(mut, seeds = [b"vault", config], bump)]
    pub vault: &'info mut UncheckedAccount,
    pub system_program: &'info Program<System>,
}

impl<'info> Deposit<'info> {
    #[inline(always)]
    pub fn deposit(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(self.depositor, self.vault, amount)
            .invoke()
    }
}
