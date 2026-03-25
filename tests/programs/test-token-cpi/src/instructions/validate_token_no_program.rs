use {
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token},
};

/// Validates a token account's mint + authority using `Account<Token>`.
/// No `token_program` field needed — the program is known at compile time
/// from the `Account<Token>` type (SPL Token only).
#[derive(Accounts)]
pub struct ValidateTokenNoProgram<'info> {
    #[account(token::mint = mint, token::authority = authority)]
    pub token_account: &'info Account<Token>,
    pub mint: &'info Account<Mint>,
    pub authority: &'info Signer,
}

impl<'info> ValidateTokenNoProgram<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
