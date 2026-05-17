use {crate::state::DynamicAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicViewMutMissingField {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl DynamicViewMutMissingField {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        let tags_count_before = self.account.tags().len();

        {
            let mut guard = self.account.as_mut(self.payer.to_account_view());
            if !guard.name.set(new_name) {
                return Err(ProgramError::InvalidInstructionData);
            }
        }

        let tags_count_after = self.account.tags().len();
        if tags_count_before != tags_count_after {
            return Err(ProgramError::Custom(20));
        }

        Ok(())
    }
}
