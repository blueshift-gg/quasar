use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::ata_init, AssociatedTokenProgram, Mint, Token, TokenProgram},
};

#[derive(Accounts)]
pub struct InitAta {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init, payer = payer,
        ata_init(
            authority = wallet, mint = mint, payer = payer, token_program = token_program,
            system_program = system_program, ata_program = ata_program,
            idempotent = false,
        ),
    )]
    pub ata: Account<Token>,
    pub wallet: Signer,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}

impl InitAta {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
