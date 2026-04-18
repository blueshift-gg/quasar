use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct CloseTokenInterface {
    pub authority: Signer,
    #[account(mut, close = destination, token::mint = mint, token::authority = authority)]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    /// CHECK: destination may alias authority (close sends lamports to it).
    #[account(mut, dup)]
    pub destination: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
}

impl CloseTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
