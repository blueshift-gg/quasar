use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct MutCheck<'info> {
    #[account(mut)]
    pub account: &'info mut Account<SimpleAccount>,
}

impl<'info> MutCheck<'info> {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        self.account.set(&SimpleAccount {
            authority: self.account.authority,
            value: new_value,
            bump: self.account.bump,
        })
    }
}
