use quasar_core::prelude::*;

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct ClosePda<'info> {
    pub authority: &'info mut Signer,
    #[account(
        has_one = authority,
        close = authority,
        seeds = [b"user", authority],
        bump = user.bump
    )]
    pub user: &'info mut Account<UserAccount>,
}

impl<'info> ClosePda<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
