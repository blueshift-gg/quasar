use {
    crate::state::{IntakeQueue, IntakeQueueInner, SIDE_A},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitConstSeed {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(mut, init, payer = payer, address = IntakeQueue::seeds(authority.address(), SIDE_A))]
    pub intake: Account<IntakeQueue>,
    pub system_program: Program<SystemProgram>,
}

impl InitConstSeed {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitConstSeedBumps) -> Result<(), ProgramError> {
        self.intake.set_inner(IntakeQueueInner {
            authority: *self.authority.address(),
            side: SIDE_A,
            bump: bumps.intake,
        });
        Ok(())
    }
}
