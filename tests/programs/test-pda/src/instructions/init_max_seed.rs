use quasar_core::prelude::*;

use crate::state::MaxSeedAccount;

#[derive(Accounts)]
pub struct InitMaxSeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"abcdefghijklmnopqrstuvwxyz012345"], bump)]
    pub max_seed: &'info mut Account<MaxSeedAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitMaxSeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitMaxSeedBumps) -> Result<(), ProgramError> {
        self.max_seed.set_inner(bumps.max_seed);
        Ok(())
    }
}
