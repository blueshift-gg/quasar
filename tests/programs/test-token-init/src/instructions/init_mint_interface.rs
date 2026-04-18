use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct InitMintInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, mint::decimals = 6, mint::authority = mint_authority)]
    pub mint: InterfaceAccount<Mint>,
    pub mint_authority: Signer,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,
}

impl InitMintInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
