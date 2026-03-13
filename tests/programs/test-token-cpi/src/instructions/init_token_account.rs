use {
    quasar_core::prelude::*,
    quasar_spl::{Mint, Token},
};

#[derive(Accounts)]
pub struct InitTokenAccount<'info> {
    pub payer: &'info mut Signer,
    #[account(init, token::mint = mint, token::authority = payer)]
    pub token_account: &'info mut Account<Token>,
    pub mint: &'info Account<Mint>,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitTokenAccount<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
