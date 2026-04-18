use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct RevokeInterface {
    pub authority: Signer,
    #[account(mut)]
    pub source: InterfaceAccount<Token>,
    pub token_program: Interface<TokenInterface>,
}

impl RevokeInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .revoke(&self.source, &self.authority)
            .invoke()
    }
}
