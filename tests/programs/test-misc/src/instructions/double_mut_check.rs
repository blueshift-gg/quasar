use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct DoubleMutCheck {
    pub signer: Signer,
    #[account(mut)]
    pub account_a: Account<SimpleAccount>,
    #[account(mut)]
    pub account_b: Account<SimpleAccount>,
}

impl DoubleMutCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
