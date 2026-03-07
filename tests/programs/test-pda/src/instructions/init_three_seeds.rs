use quasar_core::prelude::*;

use crate::state::ThreeSeedAccount;

#[derive(Accounts)]
pub struct InitThreeSeeds<'info> {
    pub payer: &'info mut Signer,
    pub first: &'info Signer,
    pub second: &'info Signer,
    #[account(init, payer = payer, seeds = [b"triple", first, second], bump)]
    pub triple: &'info mut Account<ThreeSeedAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitThreeSeeds<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitThreeSeedsBumps) -> Result<(), ProgramError> {
        self.triple
            .set_inner(*self.first.address(), *self.second.address(), bumps.triple);
        Ok(())
    }
}
