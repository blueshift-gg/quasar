use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{Mint2022, Token2022, Token2022Program, TokenCpi},
};

#[derive(Accounts)]
pub struct TransferCheckedT22 {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token2022>,
    pub mint: Account<Mint2022>,
    #[account(mut)]
    pub to: Account<Token2022>,
    pub token_program: Program<Token2022Program>,
}

impl TransferCheckedT22 {
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
