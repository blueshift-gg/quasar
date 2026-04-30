use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::ata_init, AssociatedTokenProgram, Mint2022, Token2022, Token2022Program},
};

#[derive(Accounts)]
pub struct InitIfNeededAtaT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init(idempotent), payer = payer,
        ata_init(
            authority = wallet, mint = mint, payer = payer, token_program = token_program,
            system_program = system_program, ata_program = ata_program, idempotent = true,
        ),
    )]
    pub ata: Account<Token2022>,
    pub wallet: Signer,
    pub mint: Account<Mint2022>,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}

impl InitIfNeededAtaT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
