use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionU64None {
    pub signer: Signer,
}

impl OptionU64None {
    #[inline(always)]
    pub fn handler(&self, value: Option<u64>) -> Result<(), ProgramError> {
        require!(value.is_none(), ProgramError::InvalidInstructionData);
        Ok(())
    }
}
