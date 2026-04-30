use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint2022, Token2022, Token2022Program, TokenCpi},
};

#[derive(Accounts)]
pub struct BurnT22 {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token2022>,
    #[account(mut)]
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
}

impl BurnT22 {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .burn(&self.from, &self.mint, &self.authority, amount)
            .invoke()
    }
}
