use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint, Token, TokenProgram},
};

#[derive(Accounts)]
pub struct ValidateTokenCheck {
    #[account(token(mint = mint, authority = authority, token_program = token_program))]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    pub authority: Signer,
    pub token_program: Program<TokenProgram>,
}

impl ValidateTokenCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
