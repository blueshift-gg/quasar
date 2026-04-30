use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionU64Some {
    pub signer: Signer,
}

impl OptionU64Some {
    #[inline(always)]
    pub fn handler(&self, value: Option<u64>) -> Result<(), ProgramError> {
        require!(value == Some(42), ProgramError::InvalidInstructionData);
        Ok(())
    }
}
