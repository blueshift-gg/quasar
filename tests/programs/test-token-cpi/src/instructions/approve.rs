use quasar_core::prelude::*;
use quasar_spl::{Token, TokenCpi, TokenProgram};

#[derive(Accounts)]
pub struct Approve<'info> {
    pub authority: &'info Signer,
    pub source: &'info mut Account<Token>,
    pub delegate: &'info UncheckedAccount,
    pub token_program: &'info TokenProgram,
}

impl<'info> Approve<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(self.source, self.delegate, self.authority, amount)
            .invoke()
    }
}
