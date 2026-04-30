use {crate::state::DynBytesAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynBytesCheck {
    pub account: Account<DynBytesAccount>,
}

impl DynBytesCheck {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let data = self.account.data();
        if data.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}
