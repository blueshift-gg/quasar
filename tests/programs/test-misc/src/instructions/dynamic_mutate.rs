use {crate::state::DynamicAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicMutate {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl DynamicMutate {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        let mut guard = self.account.as_mut(self.payer.to_account_view());
        if !guard.name.set(new_name) {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}
