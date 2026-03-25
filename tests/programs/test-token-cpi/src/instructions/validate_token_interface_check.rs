use {
    quasar_lang::prelude::*,
    quasar_spl::{InterfaceAccount, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateTokenInterfaceCheck<'info> {
    #[account(token::mint = mint, token::authority = authority)]
    pub token_account: &'info InterfaceAccount<Token>,
    pub mint: &'info InterfaceAccount<Mint>,
    pub authority: &'info Signer,
    pub token_program: &'info Interface<TokenInterface>,
}

impl<'info> ValidateTokenInterfaceCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
