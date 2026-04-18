use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct InitIfNeededMintInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init_if_needed, mint::decimals = 6, mint::authority = mint_authority)]
    pub mint: InterfaceAccount<Mint>,
    pub mint_authority: Signer,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,
}

impl InitIfNeededMintInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
