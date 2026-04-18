use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct ApproveInterface {
    pub authority: Signer,
    #[account(mut)]
    pub source: InterfaceAccount<Token>,
    pub delegate: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
}

impl ApproveInterface {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(&self.source, &self.delegate, &self.authority, amount)
            .invoke()
    }
}
