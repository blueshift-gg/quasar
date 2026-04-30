use {crate::state::DynamicAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct MutateThenReadback {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl MutateThenReadback {
    #[inline(always)]
    pub fn handler(&mut self, expected_tags_count: u8, new_name: &str) -> Result<(), ProgramError> {
        // Mutate via as_mut guard — only change name, tags preserved automatically
        {
            let mut guard = self.account.as_mut(self.payer.to_account_view());
            if !guard.name.set(new_name) {
                return Err(ProgramError::InvalidInstructionData);
            }
        }

        // Read back from account data to verify the save worked
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
