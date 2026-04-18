use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct InterfaceTransfer {
    pub authority: Signer,
    #[account(mut)]
    pub from: InterfaceAccount<Token>,
    #[account(mut)]
    pub to: InterfaceAccount<Token>,
    pub token_program: Interface<TokenInterface>,
}

impl InterfaceTransfer {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .transfer(&self.from, &self.to, &self.authority, amount)
            .invoke()
    }
}
