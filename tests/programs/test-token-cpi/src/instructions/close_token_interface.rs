use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};

#[derive(Accounts)]
pub struct CloseTokenInterface {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    /// Test-only duplicate destination; close may send lamports to authority.
    #[account(mut, dup)]
    pub destination: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
}

impl CloseTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
