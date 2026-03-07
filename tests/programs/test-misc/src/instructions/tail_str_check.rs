use quasar_core::prelude::*;

use crate::state::TailStrAccount;

#[derive(Accounts)]
pub struct TailStrCheck<'info> {
    pub account: Account<TailStrAccount<'info>>,
}

impl<'info> TailStrCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let label = self.account.label();
        if label.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}
