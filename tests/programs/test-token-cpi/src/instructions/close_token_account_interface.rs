use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

#[derive(Accounts)]
pub struct CloseTokenAccountInterface {
    #[account(mut)]
    pub account: InterfaceAccount<Token>,
    #[account(mut)]
    pub destination: Signer,
    /// Duplicate signer used when authority and destination alias.
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
