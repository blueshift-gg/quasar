use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionAddressSome {
    pub signer: Signer,
}

impl OptionAddressSome {
    #[inline(always)]
    pub fn handler(&self, addr: Option<Address>) -> Result<(), ProgramError> {
        require!(addr.is_some(), ProgramError::InvalidInstructionData);
        Ok(())
    }
}
