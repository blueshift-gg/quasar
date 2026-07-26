use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct SignerPair {
    pub first: Signer,
    pub second: Signer,
}

#[derive(Accounts)]
pub struct CompositeGroup {
    pub payer: Signer,
    #[account(group)]
    pub pair: SignerPair,
}

impl CompositeGroup {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
