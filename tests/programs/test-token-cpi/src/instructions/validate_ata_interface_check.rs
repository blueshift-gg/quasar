use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::associated_token, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateAtaInterfaceCheck {
    #[account(associated_token(mint = mint, authority = wallet, token_program = token_program))]
    pub ata: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub wallet: Signer,
    pub token_program: Interface<TokenInterface>,
}

impl ValidateAtaInterfaceCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
