use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::mint, Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateMintInterfaceCheck {
    #[account(mint(authority = mint_authority, decimals = 6, freeze_authority = None, token_program = token_program))]
    pub mint: InterfaceAccount<Mint>,
    pub mint_authority: Signer,
    pub token_program: Interface<TokenInterface>,
}

impl ValidateMintInterfaceCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
