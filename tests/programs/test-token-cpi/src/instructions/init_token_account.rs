use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint, Token, TokenProgram},
};

#[derive(Accounts)]
pub struct InitTokenAccount {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl InitTokenAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
