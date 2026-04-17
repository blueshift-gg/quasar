use {
    crate::state::DynamicAccount,
    quasar_lang::{prelude::*, sysvars::Sysvar as _},
};

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
        let rent = Rent::get()?;
        let mut guard = self.account.compact_mut(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );
        if !guard.name.set(new_name) {
            return Err(ProgramError::InvalidInstructionData);
        }
        guard.save()?;
        Ok(())
    }
}
