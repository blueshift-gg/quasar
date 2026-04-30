use {
    crate::state::{MaxSeedAccount, MaxSeedAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitMaxSeedLength {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = MaxSeedAccount::seeds())]
    pub max_seed: Account<MaxSeedAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitMaxSeedLength {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitMaxSeedLengthBumps) -> Result<(), ProgramError> {
        self.max_seed.set_inner(MaxSeedAccountInner {
            bump: bumps.max_seed,
        });
        Ok(())
    }
}
