use quasar_core::prelude::*;

use crate::state::{ConfigAccount, ConfigAccountInit};

#[derive(Accounts)]
pub struct InitLiteralSeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"config"], bump)]
    pub config: &'info mut Account<ConfigAccount>,
    pub system_program: &'info SystemProgram,
}

impl<'info> InitLiteralSeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitLiteralSeedBumps) -> Result<(), ProgramError> {
        self.config.set(&ConfigAccountInit { bump: bumps.config })
    }
}
