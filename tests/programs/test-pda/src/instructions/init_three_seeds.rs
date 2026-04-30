use {
    crate::state::{ThreeSeedAccount, ThreeSeedAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitThreeSeeds {
    #[account(mut)]
    pub payer: Signer,
    pub first: Signer,
    pub second: Signer,
    #[account(mut, init, payer = payer, address = ThreeSeedAccount::seeds(first.address(), second.address()))]
    pub triple: Account<ThreeSeedAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitThreeSeeds {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitThreeSeedsBumps) -> Result<(), ProgramError> {
        self.triple.set_inner(ThreeSeedAccountInner {
            first: *self.first.address(),
            second: *self.second.address(),
            bump: bumps.triple,
        });
        Ok(())
    }
}
