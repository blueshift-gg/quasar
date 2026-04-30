use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct Burn {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token>,
    #[account(mut)]
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
}

impl Burn {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .burn(&self.from, &self.mint, &self.authority, amount)
            .invoke()
    }
}
