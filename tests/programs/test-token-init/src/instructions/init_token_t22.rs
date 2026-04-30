use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::token, Mint2022, Token2022, Token2022Program},
};

#[derive(Accounts)]
pub struct InitTokenT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: Account<Token2022>,
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
}

impl InitTokenT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
