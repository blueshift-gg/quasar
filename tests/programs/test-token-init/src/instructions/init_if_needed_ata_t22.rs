use {
    quasar_lang::prelude::*,
    quasar_spl::{AssociatedTokenProgram, Mint2022, Token2022},
};

#[derive(Accounts)]
pub struct InitIfNeededAtaT22<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, associated_token::mint = mint, associated_token::authority = wallet)]
    pub ata: &'info mut Account<Token2022>,
    pub wallet: &'info Signer,
    pub mint: &'info Account<Mint2022>,
    pub token_program: &'info Program<Token2022>,
    pub system_program: &'info Program<System>,
    pub ata_program: &'info Program<AssociatedTokenProgram>,
}

impl<'info> InitIfNeededAtaT22<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
