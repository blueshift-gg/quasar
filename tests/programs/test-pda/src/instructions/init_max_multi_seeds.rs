use {
    quasar_derive::{Accounts, Seeds},
    quasar_lang::prelude::*,
};

#[derive(Seeds)]
#[seeds(b"max_multi_seeds")]
pub struct MaxMultiSeedsPda;

#[derive(Accounts)]
pub struct InitMaxMultiSeeds {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(address = MaxMultiSeedsPda::seeds())]
    pub complex: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

impl InitMaxMultiSeeds {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        Ok(())
    }
}
