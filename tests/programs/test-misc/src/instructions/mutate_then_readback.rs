use {crate::state::DynamicAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct MutateThenReadback<'info> {
    #[account(mut)]
    pub account: Account<DynamicAccount<'info>>,
    #[account(mut)]
    pub payer: &'info mut Signer,
    pub system_program: &'info Program<System>,
}

impl<'info> MutateThenReadback<'info> {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str, expected_tags_count: u8) -> Result<(), ProgramError> {
        self.account.set_name(self.payer, new_name)?;

        let name = self.account.name();
        if name.len() != new_name.len() {
            return Err(ProgramError::Custom(10));
        }
        if name.as_bytes() != new_name.as_bytes() {
            return Err(ProgramError::Custom(11));
        }

        let tags = self.account.tags();
        if tags.len() != expected_tags_count as usize {
            return Err(ProgramError::Custom(12));
        }

        Ok(())
    }
}
