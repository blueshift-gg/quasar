use {
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token},
};

#[derive(Accounts)]
pub struct ValidateTokenCheck<'info> {
    #[account(token::mint = mint, token::authority = authority)]
    pub token_account: &'info Account<Token>,
    pub mint: &'info Account<Mint>,
    pub authority: &'info Signer,
    pub token_program: &'info Program<Token>,
}

impl<'info> ValidateTokenCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
