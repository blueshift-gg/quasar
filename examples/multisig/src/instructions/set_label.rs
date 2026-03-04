use quasar_core::prelude::*;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct SetLabel<'info> {
    pub creator: &'info mut Signer,
    #[account(
        mut,
        has_one = creator,
        seeds = [b"multisig", creator],
        bump = config.bump
    )]
    pub config: &'info mut Account<MultisigConfig<'info>>,
    pub system_program: &'info Program<System>,
}

impl<'info> SetLabel<'info> {
    #[inline(always)]
    pub fn update_label(&mut self, label: &str) -> Result<(), ProgramError> {
        self.config.set_label(self.creator, label)
    }
}
