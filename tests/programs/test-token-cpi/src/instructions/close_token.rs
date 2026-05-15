use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

/// Closes a token account through the account-exit epilogue.
#[derive(Accounts)]
pub struct CloseToken {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    /// Test-only duplicate destination; close may send lamports to authority.
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
