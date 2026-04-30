use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct MintTo {
    pub authority: Signer,
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<Token>,
    pub token_program: Program<TokenProgram>,
}

impl MintTo {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .mint_to(&self.mint, &self.to, &self.authority, amount)
            .invoke()
    }
}
