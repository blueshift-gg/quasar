use {
    quasar_core::prelude::*,
    quasar_spl::{InterfaceAccount, Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct InterfaceTransfer<'info> {
    pub authority: &'info Signer,
    pub from: &'info mut InterfaceAccount<Token>,
    pub to: &'info mut InterfaceAccount<Token>,
    pub token_program: &'info Interface<TokenInterface>,
}

impl<'info> InterfaceTransfer<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .transfer(self.from, self.to, self.authority, amount)
            .invoke()
    }
}
