use quasar_core::prelude::*;

use crate::state::SimpleAccount;

#[derive(Accounts)]
pub struct DoubleMutCheck<'info> {
    pub signer: &'info Signer,
    #[account(mut)]
    pub account_a: &'info mut Account<SimpleAccount>,
    #[account(mut)]
    pub account_b: &'info mut Account<SimpleAccount>,
}

impl<'info> DoubleMutCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
