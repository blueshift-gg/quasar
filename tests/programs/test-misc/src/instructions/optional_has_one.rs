use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct OptionalHasOne<'info> {
    pub authority: &'info Signer,
    #[account(has_one = authority)]
    pub account: Option<&'info Account<SimpleAccount>>,
}

impl<'info> OptionalHasOne<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
