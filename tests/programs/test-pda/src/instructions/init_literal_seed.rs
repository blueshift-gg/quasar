use quasar_core::prelude::*;

use crate::state::ConfigAccount;

#[derive(Accounts)]
pub struct InitLiteralSeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"config"], bump)]
    pub config: &'info mut Account<ConfigAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitLiteralSeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitLiteralSeedBumps) -> Result<(), ProgramError> {
        self.config.set(&ConfigAccount { bump: bumps.config })
    }
}
