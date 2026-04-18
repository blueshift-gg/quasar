use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct CloseTokenAccountInterface {
    #[account(mut)]
    pub account: InterfaceAccount<Token>,
    #[account(mut)]
    pub destination: Signer,
    /// CHECK: authority may equal destination when the signer is closing to
    /// themselves.
    #[account(dup)]
    pub authority: Signer,
    pub token_program: Interface<TokenInterface>,
}

impl CloseTokenAccountInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .close_account(&self.account, &self.destination, &self.authority)
            .invoke()
    }
}
