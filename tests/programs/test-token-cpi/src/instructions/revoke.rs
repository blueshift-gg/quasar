use quasar_core::prelude::*;
use quasar_spl::{Token, TokenAccount, TokenCpi};

#[derive(Accounts)]
pub struct Revoke<'info> {
    pub authority: &'info Signer,
    pub source: &'info mut Account<TokenAccount>,
    pub token_program: &'info Program<Token>,
}

impl<'info> Revoke<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .revoke(self.source, self.authority)
            .invoke()
    }
}
