use {crate::state::TailStrAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct TailStrCheck {
    pub account: Account<TailStrAccount>,
}

impl TailStrCheck {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let label = self.account.label();
        if label.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}
