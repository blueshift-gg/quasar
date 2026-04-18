use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct InitIfNeededTokenInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init_if_needed, token::mint = mint, token::authority = payer)]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<System>,
}

impl InitIfNeededTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
