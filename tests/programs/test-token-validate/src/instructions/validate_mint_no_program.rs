use {
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
    quasar_spl::{ops::mint, Mint, TokenProgram},
};

/// V1 had no `token_program` field (program inferred at compile time).
/// V2 requires an explicit token_program field.
#[derive(Accounts)]
pub struct ValidateMintNoProgram {
    #[account(mint(authority = mint_authority, decimals = 6, freeze_authority = None, token_program = token_program))]
    pub mint: Account<Mint>,
    pub mint_authority: Signer,
    pub token_program: Program<TokenProgram>,
}

impl ValidateMintNoProgram {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
