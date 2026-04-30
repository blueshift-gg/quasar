use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint, Token, TokenProgram},
};

#[derive(Seeds)]
#[seeds(b"token", payer: Address)]
pub struct TokenPda;

#[derive(Accounts)]
pub struct InitTokenPda {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        address = TokenPda::seeds(payer.address()),
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl InitTokenPda {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
