use quasar_core::prelude::*;

#[derive(Accounts)]
pub struct DynamicInstructionCheck<'info> {
    pub authority: &'info Signer,
}

impl<'info> DynamicInstructionCheck<'info> {
    #[inline(always)]
    pub fn handler(&self, _name: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}
