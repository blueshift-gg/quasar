use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct Revoke {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token>,
    pub token_program: Program<TokenProgram>,
}

impl Revoke {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .revoke(&self.source, &self.authority)
            .invoke()
    }
}
