use {crate::state::DynamicAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicViewMut {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl DynamicViewMut {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str, new_tags: &[Address]) -> Result<(), ProgramError> {
        {
            let mut guard = self.account.as_mut(self.payer.to_account_view());
            if !guard.name.set(new_name) {
                return Err(ProgramError::InvalidInstructionData);
            }
            if !guard.tags.set_from_slice(new_tags) {
                return Err(ProgramError::InvalidInstructionData);
            }
        }

        if self.account.name() != new_name {
            return Err(ProgramError::Custom(13));
        }
        if self.account.tags() != new_tags {
            return Err(ProgramError::Custom(14));
        }

        Ok(())
    }
}
