use {
    crate::state::{EmptySeedAccount, EmptySeedAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitEmptySeed {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = EmptySeedAccount::seeds())]
    pub empty: Account<EmptySeedAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitEmptySeed {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitEmptySeedBumps) -> Result<(), ProgramError> {
        self.empty
            .set_inner(EmptySeedAccountInner { bump: bumps.empty });
        Ok(())
    }
}
