use {
    crate::state::ClockFullSnapshot,
    quasar_core::{
        prelude::*,
        sysvars::{clock::Clock, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadClockFull<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"clock_full"], bump)]
    pub snapshot: &'info mut Account<ClockFullSnapshot>,
    pub system_program: &'info Program<System>,
}

impl<'info> ReadClockFull<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        self.snapshot.set_inner(
            clock.slot.get(),
            clock.epoch_start_timestamp.get(),
            clock.epoch.get(),
            clock.leader_schedule_epoch.get(),
            clock.unix_timestamp.get(),
        );
        Ok(())
    }
}
