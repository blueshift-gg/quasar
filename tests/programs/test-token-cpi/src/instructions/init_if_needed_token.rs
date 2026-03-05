use quasar_core::prelude::*;
use quasar_spl::{Mint, Token, TokenAccount};

#[derive(Accounts)]
pub struct InitIfNeededToken<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, token::mint = mint, token::authority = payer)]
    pub token_account: &'info mut Account<TokenAccount>,
    pub mint: &'info Account<Mint>,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitIfNeededToken<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
