use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::token, Mint, Token, TokenInterface},
};

#[derive(Accounts)]
pub struct InitTokenInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        token(mint = mint, authority = payer, token_program = token_program),
    )]
    pub token_account: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

impl InitTokenInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
