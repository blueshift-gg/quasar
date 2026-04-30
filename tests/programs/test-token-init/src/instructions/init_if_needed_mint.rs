use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::mint, Mint, TokenProgram},
};

#[derive(Accounts)]
pub struct InitIfNeededMint {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init(idempotent), payer = payer,
        mint(decimals = 6, authority = mint_authority, freeze_authority = None, token_program = token_program),
    )]
    pub mint: Account<Mint>,
    pub mint_authority: Signer,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl InitIfNeededMint {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
