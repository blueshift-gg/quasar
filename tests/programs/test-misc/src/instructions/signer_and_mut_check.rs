use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct SignerAndMutCheck<'info> {
    #[account(mut)]
    pub account: &'info mut Account<SimpleAccount>,
    pub signer: &'info Signer,
}

impl<'info> SignerAndMutCheck<'info> {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        let authority = self.account.authority;
        let bump = self.account.bump;
        self.account.set_inner(authority, new_value, bump);
        Ok(())
    }
}
