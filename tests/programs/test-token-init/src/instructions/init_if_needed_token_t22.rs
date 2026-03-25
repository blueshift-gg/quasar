use {
    quasar_lang::prelude::*,
    quasar_spl::{Mint2022, Token2022},
};

#[derive(Accounts)]
pub struct InitIfNeededTokenT22<'info> {
    pub payer: &'info mut Signer,
    #[account(init_if_needed, token::mint = mint, token::authority = payer)]
    pub token_account: &'info mut Account<Token2022>,
    pub mint: &'info Account<Mint2022>,
    pub token_program: &'info Program<Token2022>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitIfNeededTokenT22<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
