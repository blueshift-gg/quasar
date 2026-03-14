use {
    quasar_core::prelude::*,
    quasar_spl::{Token, TokenCpi},
};

#[derive(Accounts)]
pub struct CloseTokenAccount<'info> {
    pub authority: &'info Signer,
    pub account: &'info mut Account<Token>,
    /// CHECK: destination may equal authority when the signer is closing to
    /// themselves.
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
