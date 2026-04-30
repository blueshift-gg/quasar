use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{
        ops::{close, sweep, token},
        Mint2022, Token2022, Token2022Program,
    },
};

#[derive(Accounts)]
pub struct SweepAndCloseT22 {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program),
        close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token2022>,
    #[account(mut)]
    pub receiver: Account<Token2022>,
    pub mint: Account<Mint2022>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub token_program: Program<Token2022Program>,
}

impl SweepAndCloseT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
