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

        // Snapshot current tags before taking &mut (CompactWriter requires all
        // dynamic fields to be set before commit).
        let mut tags_buf = [Address::default(); 2];
        let tags_len = self.account.tags().len();
        tags_buf[..tags_len].copy_from_slice(self.account.tags());

        // Mutate via compact writer — explicit commit
        {
            let mut writer = self.account.compact_mut(
                self.payer.to_account_view(),
                rent.lamports_per_byte(),
                rent.exemption_threshold_raw(),
            );
            writer.set_name(new_name)?;
            writer.set_tags(&tags_buf[..tags_len])?;
            writer.commit()?;
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
