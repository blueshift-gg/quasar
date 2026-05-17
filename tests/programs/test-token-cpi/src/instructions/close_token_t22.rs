use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

#[derive(Accounts)]
pub struct CloseTokenT22 {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub token_account: Account<Token2022>,
    pub mint: Account<Mint2022>,
    /// Test-only duplicate destination; close may send lamports to authority.
    #[account(mut, dup)]
    pub destination: UncheckedAccount,
    pub token_program: Program<Token2022Program>,
}

impl CloseTokenT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
