use quasar_core::prelude::*;
use quasar_spl::{Mint, Token, TokenCpi, TokenProgram};

#[derive(Accounts)]
pub struct Burn<'info> {
    pub authority: &'info Signer,
    pub from: &'info mut Account<Token>,
    pub mint: &'info mut Account<Mint>,
    pub token_program: &'info TokenProgram,
}

impl<'info> Burn<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .burn(self.from, self.mint, self.authority, amount)
            .invoke()
    }
}
