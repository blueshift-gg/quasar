use quasar_core::prelude::*;
use quasar_spl::{Token, TokenAccount, TokenCpi};

#[derive(Accounts)]
pub struct Approve<'info> {
    pub authority: &'info Signer,
    pub source: &'info mut Account<TokenAccount>,
    pub delegate: &'info UncheckedAccount,
    pub token_program: &'info Program<Token>,
}

impl<'info> Approve<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(self.source, self.delegate, self.authority, amount)
            .invoke()
    }
}
