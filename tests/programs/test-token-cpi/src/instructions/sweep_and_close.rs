use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

/// Transfers all tokens, then closes the account.
#[derive(Accounts)]
pub struct SweepAndClose {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program),
        token_close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token>,
    #[account(mut)]
    pub receiver: Account<Token>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}

impl SweepAndClose {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
