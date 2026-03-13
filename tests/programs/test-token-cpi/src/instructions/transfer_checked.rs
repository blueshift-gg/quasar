use {
    quasar_core::prelude::*,
    quasar_spl::{Mint, Token, TokenCpi},
};

#[derive(Accounts)]
pub struct TransferChecked<'info> {
    pub authority: &'info Signer,
    pub from: &'info mut Account<Token>,
    pub mint: &'info Account<Mint>,
    pub to: &'info mut Account<Token>,
    pub token_program: &'info Program<Token>,
}

impl<'info> TransferChecked<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64, decimals: u8) -> Result<(), ProgramError> {
        self.token_program
            .transfer_checked(
                self.from,
                self.mint,
                self.to,
                self.authority,
                amount,
                decimals,
            )
            .invoke()
    }
}
