use {
    quasar_core::prelude::*,
    quasar_spl::{Mint, Token},
};

#[derive(Accounts)]
pub struct InitMintAccount<'info> {
    pub payer: &'info mut Signer,
    #[account(init, mint::decimals = 6, mint::authority = mint_authority)]
    pub mint: &'info mut Account<Mint>,
    pub mint_authority: &'info Signer,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitMintAccount<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
