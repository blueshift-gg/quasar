use {crate::state::TailBytesAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct TailBytesCheck {
    pub account: Account<TailBytesAccount>,
}

impl TailBytesCheck {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let data = self.account.data();
        if data.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}
