use quasar_core::prelude::*;
use quasar_spl::{AssociatedToken, Mint, TokenProgram};

#[derive(Accounts)]
pub struct ValidateAtaCheck<'info> {
    #[account(associated_token::mint = mint, associated_token::authority = wallet)]
    pub ata: &'info Account<AssociatedToken>,
    pub mint: &'info Account<Mint>,
    pub wallet: &'info Signer,
    pub token_program: &'info TokenProgram,
}

impl<'info> ValidateAtaCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
