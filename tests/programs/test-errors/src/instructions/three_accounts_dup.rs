use {quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ThreeAccountsDup {
    pub first: Signer,
    #[account(mut)]
    pub second: UncheckedAccount,
    pub third: UncheckedAccount,
}

impl ThreeAccountsDup {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
