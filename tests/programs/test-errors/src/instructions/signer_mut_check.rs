use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct SignerMutCheck {
    #[account(mut)]
    pub signer: Signer,
}

impl SignerMutCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
