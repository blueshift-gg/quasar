use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct OptionalMutAccounts {
    #[account(mut)]
    pub authority: Signer,

    #[account(mut)]
    pub first: Option<Account<SimpleAccount>>,

    #[account(mut)]
    pub second: Option<Account<SimpleAccount>>,

    #[account(mut)]
    pub third: Option<Account<SimpleAccount>>,
}

impl OptionalMutAccounts {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
