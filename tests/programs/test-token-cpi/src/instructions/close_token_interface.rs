use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{
        ops::{close, token},
        Mint, Token, TokenInterface,
    },
};

#[derive(Accounts)]
pub struct CloseTokenInterface {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    /// CHECK: destination may alias authority (close sends lamports to it).
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
