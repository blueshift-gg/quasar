use {
    crate::state::{SimpleAccount, SimpleAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SignerAndMutCheck {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    pub signer: Signer,
}

impl SignerAndMutCheck {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        let authority = self.account.authority;
        let bump = self.account.bump;
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: new_value,
            bump,
        });
        Ok(())
    }
}
