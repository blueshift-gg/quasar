use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{
        ops::{sweep, token},
        Mint, Token, TokenInterface,
    },
};

#[derive(Accounts)]
pub struct SweepTokenInterface {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program)
    )]
    pub source: InterfaceAccount<Token>,
    #[account(mut)]
    pub receiver: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
}

impl SweepTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
