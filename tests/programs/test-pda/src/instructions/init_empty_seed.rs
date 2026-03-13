use {crate::state::EmptySeedAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct InitEmptySeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b""], bump)]
    pub empty: &'info mut Account<EmptySeedAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitEmptySeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitEmptySeedBumps) -> Result<(), ProgramError> {
        self.empty.set_inner(bumps.empty);
        Ok(())
    }
}
