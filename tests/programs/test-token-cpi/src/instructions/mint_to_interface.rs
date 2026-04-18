use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct MintToInterface {
    pub authority: Signer,
    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,
    #[account(mut)]
    pub to: InterfaceAccount<Token>,
    pub token_program: Interface<TokenInterface>,
}

impl MintToInterface {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .mint_to(&self.mint, &self.to, &self.authority, amount)
            .invoke()
    }
}
