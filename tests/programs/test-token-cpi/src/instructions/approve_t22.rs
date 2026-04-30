use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token2022, Token2022Program, TokenCpi},
};

#[derive(Accounts)]
pub struct ApproveT22 {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token2022>,
    pub delegate: UncheckedAccount,
    pub token_program: Program<Token2022Program>,
}

impl ApproveT22 {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(&self.source, &self.delegate, &self.authority, amount)
            .invoke()
    }
}
