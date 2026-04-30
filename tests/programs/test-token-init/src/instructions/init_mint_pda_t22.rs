use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
    quasar_spl::{ops::mint, Mint2022, Token2022Program},
};

#[derive(Seeds)]
#[seeds(b"mint", payer: Address)]
pub struct MintPdaT22;

#[derive(Accounts)]
pub struct InitMintPdaT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        address = MintPdaT22::seeds(payer.address()),
        mint(decimals = 6, authority = payer, freeze_authority = None, token_program = token_program),
    )]
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
}

impl InitMintPdaT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
