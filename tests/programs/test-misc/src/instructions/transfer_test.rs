use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct TransferTest<'info> {
    pub from: &'info mut Signer,
    #[account(mut)]
    pub to: &'info mut UncheckedAccount,
    pub system_program: &'info Program<System>,
}

impl<'info> TransferTest<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(self.from, self.to, amount)
            .invoke()
    }
}
