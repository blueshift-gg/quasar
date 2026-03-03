use quasar_core::prelude::*;
use quasar_spl::{AssociatedToken, AssociatedTokenProgram, Mint, TokenProgram};

#[derive(Accounts)]
pub struct InitAta<'info> {
    pub payer: &'info mut Signer,
    #[account(init, associated_token::mint = mint, associated_token::authority = wallet)]
    pub ata: &'info mut Account<AssociatedToken>,
    pub wallet: &'info Signer,
    pub mint: &'info Account<Mint>,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
    pub ata_program: &'info AssociatedTokenProgram,
}

impl<'info> InitAta<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
