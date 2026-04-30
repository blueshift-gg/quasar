use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
    quasar_spl::{ops::mint, Mint, TokenProgram},
};

#[derive(Seeds)]
#[seeds(b"mint", payer: Address)]
pub struct MintPda;

#[derive(Accounts)]
pub struct InitMintPda {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        address = MintPda::seeds(payer.address()),
        mint(decimals = 6, authority = payer, freeze_authority = None, token_program = token_program),
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl InitMintPda {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
