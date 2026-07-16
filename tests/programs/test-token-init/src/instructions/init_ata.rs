use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::prelude::*};
#[derive(Accounts)]
pub struct InitAta {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut,
        init,
        associated_token(authority = wallet, mint = mint, token_program = token_program),
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
