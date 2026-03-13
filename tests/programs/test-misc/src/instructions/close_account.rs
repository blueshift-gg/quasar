use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct CloseAccount<'info> {
    pub authority: &'info mut Signer,
    #[account(
        has_one = authority,
        close = authority,
        seeds = [b"simple", authority],
        bump = account.bump
    )]
    pub account: &'info mut Account<SimpleAccount>,
}

impl<'info> CloseAccount<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
