use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token2022, Token2022Program, TokenCpi},
};

#[derive(Accounts)]
pub struct CloseTokenAccountT22 {
    #[account(mut)]
    pub account: Account<Token2022>,
    #[account(mut)]
    pub destination: Signer,
    /// CHECK: authority may equal destination when the signer is closing to
    /// themselves.
    #[account(dup)]
    pub authority: Signer,
    pub token_program: Program<Token2022Program>,
}

impl CloseTokenAccountT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .close_account(&self.account, &self.destination, &self.authority)
            .invoke()
    }
}
