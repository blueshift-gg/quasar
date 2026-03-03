use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct UpdateHasOne<'info> {
    pub authority: &'info Signer,
    #[account(has_one = authority, seeds = [b"simple", authority], bump = account.bump)]
    pub account: &'info Account<SimpleAccount>,
}

impl<'info> UpdateHasOne<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
