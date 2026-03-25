use {
    quasar_lang::prelude::*,
    quasar_spl::{Mint2022, Token2022},
};

#[derive(Accounts)]
pub struct InitIfNeededMintT22<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, mint::decimals = 6, mint::authority = mint_authority)]
    pub mint: &'info mut Account<Mint2022>,
    pub mint_authority: &'info Signer,
    pub token_program: &'info Program<Token2022>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitIfNeededMintT22<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
