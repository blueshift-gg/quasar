use {
    crate::state::DynamicAccount,
    quasar_lang::{prelude::*, sysvars::Sysvar as _},
};

#[derive(Accounts)]
pub struct DynamicViewMutMissingField {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl DynamicViewMutMissingField {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        // Snapshot the current tags count before mutation.
        let tags_count_before = self.account.tags().len();

        let rent = Rent::get()?;
        let mut guard = self.account.compact_mut(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );
        // Only set name — tags should be preserved automatically.
        if !guard.name.set(new_name) {
            return Err(ProgramError::InvalidInstructionData);
        }
        guard.save()?;

        // Verify untouched tags were preserved.
        let tags_count_after = self.account.tags().len();
        if tags_count_before != tags_count_after {
            return Err(ProgramError::Custom(20));
        }

        Ok(())
    }
}
