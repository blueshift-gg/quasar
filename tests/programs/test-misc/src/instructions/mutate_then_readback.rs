use {
    crate::state::DynamicAccount,
    quasar_lang::{prelude::*, sysvars::Sysvar as _},
};

#[derive(Accounts)]
pub struct MutateThenReadback {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl MutateThenReadback {
    #[inline(always)]
    pub fn handler(&mut self, expected_tags_count: u8, new_name: &str) -> Result<(), ProgramError> {
        let rent = Rent::get()?;

        // Mutate via guard — auto-saves on drop
        {
            let mut guard = self.account.as_dynamic_mut(
                self.payer.to_account_view(),
                rent.lamports_per_byte(),
                rent.exemption_threshold_raw(),
            );
            if !guard.name.set(new_name) {
                return Err(ProgramError::InvalidInstructionData);
            }
        } // guard dropped here → flushed to account data

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
