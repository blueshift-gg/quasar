use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::token, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateTokenInterfaceCheck {
    #[account(token(mint = mint, authority = authority, token_program = token_program))]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub authority: Signer,
    pub token_program: Interface<TokenInterface>,
}

impl ValidateTokenInterfaceCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
