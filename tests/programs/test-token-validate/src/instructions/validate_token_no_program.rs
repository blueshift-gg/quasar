use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint, Token, TokenProgram},
};

/// V1 had no `token_program` field (program inferred at compile time).
/// V2 requires an explicit token_program field.
#[derive(Accounts)]
pub struct ValidateTokenNoProgram {
    #[account(token(mint = mint, authority = authority, token_program = token_program))]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<TokenProgram>,
}

impl ValidateTokenNoProgram {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
