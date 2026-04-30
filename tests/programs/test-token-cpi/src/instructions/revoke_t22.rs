use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Token2022, Token2022Program, TokenCpi},
};

#[derive(Accounts)]
pub struct RevokeT22 {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token2022>,
    pub token_program: Program<Token2022Program>,
}

impl RevokeT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .revoke(&self.source, &self.authority)
            .invoke()
    }
}
