use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionAddressNone {
    pub signer: Signer,
}

impl OptionAddressNone {
    #[inline(always)]
    pub fn handler(&self, addr: Option<Address>) -> Result<(), ProgramError> {
        require!(addr.is_none(), ProgramError::InvalidInstructionData);
        Ok(())
    }
}
