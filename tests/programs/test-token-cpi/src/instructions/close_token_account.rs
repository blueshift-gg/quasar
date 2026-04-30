use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct CloseTokenAccount {
    #[account(mut)]
    pub account: Account<Token>,
    #[account(mut)]
    pub destination: Signer,
    /// CHECK: authority may equal destination when the signer is closing to
    /// themselves.
    #[account(dup)]
    pub authority: Signer,
    pub token_program: Program<TokenProgram>,
}

impl CloseTokenAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .close_account(&self.account, &self.destination, &self.authority)
            .invoke()
    }
}
