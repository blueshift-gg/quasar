use {
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{Mint, Token, TokenCpi, TokenInterface},
};

#[derive(Accounts)]
pub struct TransferCheckedInterface {
    pub authority: Signer,
    #[account(mut)]
    pub from: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    #[account(mut)]
    pub to: InterfaceAccount<Token>,
    pub token_program: Interface<TokenInterface>,
}

impl TransferCheckedInterface {
    #[inline(always)]
    pub fn handler(&self, amount: u64, decimals: u8) -> Result<(), ProgramError> {
        self.token_program
            .transfer_checked(
                &self.from,
                &self.mint,
                &self.to,
                &self.authority,
                amount,
                decimals,
            )
            .invoke()
    }
}
