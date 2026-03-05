use quasar_core::prelude::*;

use crate::state::TailBytesAccount;

#[derive(Accounts)]
pub struct TailBytesCheck<'info> {
    pub account: Account<TailBytesAccount<'info>>,
}

impl<'info> TailBytesCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let data = self.account.data();
        if data.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}
