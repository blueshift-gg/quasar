use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::ata_init, AssociatedTokenProgram, Mint, Token, TokenProgram},
};

#[derive(Accounts)]
pub struct InitIfNeededAta {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init(idempotent), payer = payer,
        ata_init(
            authority = wallet, mint = mint, payer = payer, token_program = token_program,
            system_program = system_program, ata_program = ata_program, idempotent = true,
        ),
    )]
    pub ata: Account<Token>,
    pub wallet: Signer,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}

impl InitIfNeededAta {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
