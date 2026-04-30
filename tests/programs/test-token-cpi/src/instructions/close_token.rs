use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{
        ops::{close, token},
        Mint, Token, TokenProgram,
    },
};

/// Tests closing a token account via the `close =` attribute.
/// The macro's epilogue routes through `AccountExit::close()` which issues
/// a CPI to the token program's `close_account` instruction.
#[derive(Accounts)]
pub struct CloseToken {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    /// CHECK: destination may alias authority (close sends lamports to it).
    #[account(mut, dup)]
    pub destination: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}

impl CloseToken {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
