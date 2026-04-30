use {
    quasar_derive::Accounts,
    quasar_lang::prelude::{InterfaceAccount, *},
    quasar_spl::{ops::mint, Mint, TokenInterface},
};

#[derive(Accounts)]
pub struct InitMintInterface {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        mint(decimals = 6, authority = mint_authority, freeze_authority = None, token_program = token_program),
    )]
    pub mint: InterfaceAccount<Mint>,
    pub mint_authority: Signer,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

impl InitMintInterface {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
