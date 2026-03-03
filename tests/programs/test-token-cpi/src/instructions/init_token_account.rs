use quasar_core::prelude::*;
use quasar_spl::{Mint, Token, TokenProgram};

#[derive(Accounts)]
pub struct InitTokenAccount<'info> {
    pub payer: &'info mut Signer,
    #[account(init, token::mint = mint, token::authority = payer)]
    pub token_account: &'info mut Account<Token>,
    pub mint: &'info Account<Mint>,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

impl<'info> InitTokenAccount<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
