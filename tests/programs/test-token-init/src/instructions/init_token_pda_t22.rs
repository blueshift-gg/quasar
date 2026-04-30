use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint2022, Token2022, Token2022Program},
};

#[derive(Seeds)]
#[seeds(b"token", payer: Address)]
pub struct TokenPdaT22;

#[derive(Accounts)]
pub struct InitTokenPdaT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        address = TokenPdaT22::seeds(payer.address()),
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: Account<Token2022>,
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
}

impl InitTokenPdaT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
