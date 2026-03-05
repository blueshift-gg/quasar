use quasar_core::prelude::*;

use crate::state::{UserAccount, UserAccountInit};

#[derive(Accounts)]
pub struct InitPubkeySeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"user", payer], bump)]
    pub user: &'info mut Account<UserAccount>,
    pub system_program: &'info SystemProgram,
}

impl<'info> InitPubkeySeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &InitPubkeySeedBumps) -> Result<(), ProgramError> {
        self.user.set(&UserAccountInit {
            authority: *self.payer.address(),
            value,
            bump: bumps.user,
        })
    }
}
