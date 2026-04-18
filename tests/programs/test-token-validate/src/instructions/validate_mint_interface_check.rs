use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateMintInterfaceCheck {
    #[account(mint::authority = mint_authority, mint::decimals = 6)]
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
