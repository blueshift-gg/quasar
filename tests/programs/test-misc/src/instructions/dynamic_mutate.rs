use quasar_core::prelude::*;

use crate::state::DynamicAccount;

#[derive(Accounts)]
pub struct DynamicMutate<'info> {
    #[account(mut)]
    pub account: &'info mut Account<DynamicAccount<'info>>,
    #[account(mut)]
    pub payer: &'info mut Signer,
    pub system_program: &'info SystemProgram,
}

impl<'info> DynamicMutate<'info> {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        self.account.set_name(self.payer, new_name)?;
        Ok(())
    }
}
