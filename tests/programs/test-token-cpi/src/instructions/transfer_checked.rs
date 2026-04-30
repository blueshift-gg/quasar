use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint, Token, TokenCpi, TokenProgram},
};

#[derive(Accounts)]
pub struct TransferChecked {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<Token>,
    pub token_program: Program<TokenProgram>,
}

impl TransferChecked {
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
