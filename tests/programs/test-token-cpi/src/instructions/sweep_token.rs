use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

/// Transfers all remaining tokens at the end of instruction processing.
#[derive(Accounts)]
pub struct SweepToken {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token>,
    #[account(mut)]
    pub receiver: Account<Token>,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
}

impl SweepToken {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
