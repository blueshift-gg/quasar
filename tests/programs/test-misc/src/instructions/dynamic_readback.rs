use {crate::state::DynamicAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicReadback {
    pub account: Account<DynamicAccount>,
}

impl DynamicReadback {
    #[inline(always)]
    pub fn handler(
        &self,
        expected_name_len: u8,
        expected_tags_count: u8,
    ) -> Result<(), ProgramError> {
        let name = self.account.name();
        if name.len() != expected_name_len as usize {
            return Err(ProgramError::Custom(1));
        }
        let tags = self.account.tags();
        if tags.len() != expected_tags_count as usize {
            return Err(ProgramError::Custom(2));
        }
        Ok(())
    }
}
