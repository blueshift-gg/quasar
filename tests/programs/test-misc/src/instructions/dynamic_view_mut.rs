use {
    crate::state::DynamicAccount,
    quasar_lang::{prelude::*, sysvars::Sysvar as _},
};

#[derive(Accounts)]
pub struct DynamicViewMut {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

impl DynamicViewMut {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str, new_tags: &[Address]) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let mut view = self.account.as_dynamic_writer(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );
        view.set_name(new_name)?;
        view.set_tags(new_tags)?;
        view.commit()?;

        if self.account.name() != new_name {
            return Err(ProgramError::Custom(13));
        }
        if self.account.tags() != new_tags {
            return Err(ProgramError::Custom(14));
        }

        Ok(())
    }
}
