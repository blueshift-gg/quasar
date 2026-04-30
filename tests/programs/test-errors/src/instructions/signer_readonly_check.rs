use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct SignerReadonlyCheck {
    pub signer: Signer,
}

impl SignerReadonlyCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
