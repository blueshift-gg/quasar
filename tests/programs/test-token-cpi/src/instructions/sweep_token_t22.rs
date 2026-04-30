use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{
        ops::{sweep, token},
        Mint2022, Token2022, Token2022Program,
    },
};

#[derive(Accounts)]
pub struct SweepTokenT22 {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token2022>,
    #[account(mut)]
    pub receiver: Account<Token2022>,
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
}

impl SweepTokenT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
