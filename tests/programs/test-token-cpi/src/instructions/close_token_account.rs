use quasar_core::prelude::*;
use quasar_spl::{Token, TokenAccount, TokenCpi};

#[derive(Accounts)]
pub struct CloseTokenAccount<'info> {
    pub authority: &'info Signer,
    pub account: &'info mut Account<TokenAccount>,
    #[account(dup)]
    pub destination: &'info mut Signer,
    pub token_program: &'info Program<Token>,
}

impl<'info> CloseTokenAccount<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .close_account(self.account, self.destination, self.authority)
            .invoke()
    }
}
