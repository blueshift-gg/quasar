use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DynamicInstructionCheck {
    pub authority: Signer,
}

impl DynamicInstructionCheck {
    #[inline(always)]
    pub fn handler(&self, _name: &str) -> Result<(), ProgramError> {
        Ok(())
    }
}
