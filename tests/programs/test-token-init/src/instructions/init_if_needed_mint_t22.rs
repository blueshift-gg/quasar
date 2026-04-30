use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::mint, Mint2022, Token2022Program},
};

#[derive(Accounts)]
pub struct InitIfNeededMintT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init(idempotent), payer = payer,
        mint(decimals = 6, authority = mint_authority, freeze_authority = None, token_program = token_program),
    )]
    pub mint: Account<Mint2022>,
    pub mint_authority: Signer,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
}

impl InitIfNeededMintT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
