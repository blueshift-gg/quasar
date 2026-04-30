use {
    crate::state::SimpleAccount,
    quasar_derive::Accounts,
    quasar_lang::{ops::close_program, prelude::*},
};

#[derive(Accounts)]
pub struct CloseAccount {
    #[account(mut)]
    pub authority: Signer,
    #[account(mut,
        has_one(authority),
        close_program(dest = authority),
        address = SimpleAccount::seeds(authority.address()),
    )]
    pub account: Account<SimpleAccount>,
}

impl CloseAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
