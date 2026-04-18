use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateMintWithFreezeInterfaceCheck {
    #[account(mint::authority = mint_authority, mint::decimals = 6, mint::freeze_authority = freeze_authority)]
    pub mint: InterfaceAccount<Mint>,
    pub mint_authority: Signer,
    pub freeze_authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
}

impl ValidateMintWithFreezeInterfaceCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
