use {
    crate::state::UserAccount,
    quasar_derive::Accounts,
    quasar_lang::{ops::close_program, prelude::*},
};

#[derive(Accounts)]
pub struct ClosePda {
    #[account(mut)]
    pub authority: Signer,
    #[account(
        mut,
        has_one(authority),
        address = UserAccount::seeds(authority.address()),
        close_program(dest = authority)
    )]
    pub user: Account<UserAccount>,
}

impl ClosePda {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
