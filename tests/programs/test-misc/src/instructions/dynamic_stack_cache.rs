use {
    crate::state::DynamicAccount,
    quasar_lang::{prelude::*, sysvars::Sysvar as _},
};

#[derive(Accounts)]
pub struct DynamicStackCache {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl DynamicStackCache {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        let rent = Rent::get()?;

        // Snapshot current tags before taking &mut (CompactWriter requires all
        // dynamic fields to be set before commit).
        let mut tags_buf = [Address::default(); 2];
        let tags_len = self.account.tags().len();
        tags_buf[..tags_len].copy_from_slice(self.account.tags());

        let mut writer = self.account.compact_mut(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );
        writer.set_name(new_name)?;
        writer.set_tags(&tags_buf[..tags_len])?;
        writer.commit()?;
        Ok(())
    }
}
