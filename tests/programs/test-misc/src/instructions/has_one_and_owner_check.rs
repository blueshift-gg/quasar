use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct HasOneAndOwnerCheck<'info> {
    pub authority: &'info Signer,
    #[account(has_one = authority)]
    pub account: &'info Account<SimpleAccount>,
}

impl<'info> HasOneAndOwnerCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
