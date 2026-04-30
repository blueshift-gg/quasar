use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct Approve {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token>,
    pub delegate: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}

impl Approve {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(&self.source, &self.delegate, &self.authority, amount)
            .invoke()
    }
}
