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
        let rent = Rent::get()?;
        let mut view = self.account.as_dynamic_writer(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );
        view.set_name(new_name)?;

        match view.commit() {
            Err(err) if err == QuasarError::DynWriterFieldNotSet.into() => Ok(()),
            Err(err) => Err(err),
            Ok(()) => Err(ProgramError::InvalidInstructionData),
        }
    }
}
