use {
    crate::state::{IntakeQueue, IntakeQueueInner, SIDE_A},
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitConstSeed {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(mut, init, payer = payer, seeds = IntakeQueue::seeds(authority, SIDE_A), bump)]
    pub intake: Account<IntakeQueue>,
    pub system_program: Program<System>,
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
