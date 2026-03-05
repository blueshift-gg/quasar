use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct SpaceOverride<'info> {
    pub payer: &'info mut Signer,
    #[account(init, space = 100, seeds = [b"spacetest", payer], bump)]
    pub account: &'info mut Account<SimpleAccount>,
    pub system_program: &'info SystemProgram,
}

impl<'info> SpaceOverride<'info> {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &SpaceOverrideBumps) -> Result<(), ProgramError> {
        self.account
            .set_inner(*self.payer.address(), value, bumps.account);
        Ok(())
    }
}
