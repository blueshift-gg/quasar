use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct BurnInterface {
    pub authority: Signer,
    #[account(mut)]
    pub from: InterfaceAccount<Token>,
    #[account(mut)]
    pub mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
}

impl BurnInterface {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .burn(&self.from, &self.mint, &self.authority, amount)
            .invoke()
    }
}
