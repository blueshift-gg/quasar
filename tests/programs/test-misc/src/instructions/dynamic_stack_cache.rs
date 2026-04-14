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

        // RAII guard: loads dynamic fields into stack copies.
        // Fixed fields still accessed zero-copy via Deref/DerefMut.
        // On drop → auto-saves all dynamic fields in one batched write.
        let mut guard = self.account.as_dynamic_mut(
            self.payer.to_account_view(),
            rent.lamports_per_byte(),
            rent.exemption_threshold_raw(),
        );

        // Mutate dynamic field on stack (free — no memmove, no realloc)
        if !guard.name.set(new_name) {
            return Err(ProgramError::InvalidInstructionData);
        }

        // guard drops here → auto-save flushes to account data
        Ok(())
    }
}
