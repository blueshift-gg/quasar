use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct InitIfNeeded<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, payer = payer, seeds = [b"simple", payer], bump)]
    pub account: &'info mut Account<SimpleAccount>,
    pub system_program: &'info SystemProgram,
}

impl<'info> InitIfNeeded<'info> {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &InitIfNeededBumps) -> Result<(), ProgramError> {
        self.account.set(&SimpleAccount {
            authority: *self.payer.address(),
            value,
            bump: bumps.account,
        })
    }
}
