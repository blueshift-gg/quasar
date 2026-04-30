use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::token, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct InitIfNeededTokenInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init(idempotent), payer = payer,
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

impl InitIfNeededTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
