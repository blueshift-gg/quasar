use {
    quasar_lang::prelude::*,
    quasar_spl::{InterfaceAccount, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateAtaInterfaceCheck<'info> {
    #[account(associated_token::mint = mint, associated_token::authority = wallet)]
    pub ata: &'info InterfaceAccount<Token>,
    pub mint: &'info InterfaceAccount<Mint>,
    pub wallet: &'info Signer,
    pub token_program: &'info Interface<TokenInterface>,
}

impl<'info> ValidateAtaInterfaceCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
