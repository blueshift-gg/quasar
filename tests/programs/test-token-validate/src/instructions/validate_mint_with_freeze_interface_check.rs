use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::mint, Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct ValidateMintWithFreezeInterfaceCheck {
    #[account(mint(authority = mint_authority, decimals = 6, freeze_authority = Some(freeze_authority), token_program = token_program))]
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
