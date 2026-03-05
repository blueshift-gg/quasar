use quasar_core::prelude::*;
use quasar_spl::{Mint, Token, TokenAccount, TokenCpi};

#[derive(Accounts)]
pub struct MintTo<'info> {
    pub authority: &'info Signer,
    pub mint: &'info mut Account<Mint>,
    pub to: &'info mut Account<TokenAccount>,
    pub token_program: &'info Program<Token>,
}

impl<'info> MintTo<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .mint_to(self.mint, self.to, self.authority, amount)
            .invoke()
    }
}
