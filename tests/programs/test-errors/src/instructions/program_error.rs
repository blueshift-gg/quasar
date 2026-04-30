use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ProgramErrorIx {
    pub signer: Signer,
}

impl ProgramErrorIx {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Err(ProgramError::InvalidAccountData)
    }
}
