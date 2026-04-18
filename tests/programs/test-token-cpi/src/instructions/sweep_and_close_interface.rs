use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct SweepAndCloseInterface {
    pub authority: Signer,
    #[account(mut, sweep = receiver, close = destination, token::mint = mint, token::authority = authority)]
    pub source: InterfaceAccount<Token>,
    #[account(mut)]
    pub receiver: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
}

impl SweepAndCloseInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
