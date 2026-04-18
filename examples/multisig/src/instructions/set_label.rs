use {crate::state::MultisigConfig, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct SetLabel {
    #[account(mut)]
    pub creator: Signer,
    #[account(
        mut,
        has_one = creator,
        seeds = MultisigConfig::seeds(creator),
        bump = config.bump
    )]
    pub config: Account<MultisigConfig>,
    pub system_program: Program<System>,
}

impl SetLabel {
    #[inline(always)]
    pub fn update_label(&mut self, label: &str) -> Result<(), ProgramError> {
        let mut guard = self.config.as_mut(self.creator.to_account_view());
        if !guard.label.set(label) {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}
