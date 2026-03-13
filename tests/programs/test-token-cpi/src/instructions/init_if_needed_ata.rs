use {
    quasar_core::prelude::*,
    quasar_spl::{AssociatedToken, AssociatedTokenProgram, Mint, Token},
};

#[derive(Accounts)]
pub struct InitIfNeededAta<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, associated_token::mint = mint, associated_token::authority = wallet)]
    pub ata: &'info mut Account<AssociatedToken>,
    pub wallet: &'info Signer,
    pub mint: &'info Account<Mint>,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
    pub ata_program: &'info Program<AssociatedTokenProgram>,
}

impl<'info> InitIfNeededAta<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
