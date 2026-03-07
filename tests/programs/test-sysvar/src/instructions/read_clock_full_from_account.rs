use quasar_core::prelude::*;
use quasar_core::sysvars::clock::Clock;

use crate::state::ClockFullSnapshot;

#[derive(Accounts)]
pub struct ReadClockFullFromAccount<'info> {
    pub _payer: &'info Signer,
    #[account(mut)]
    pub snapshot: &'info mut Account<ClockFullSnapshot>,
    pub clock: &'info Sysvar<Clock>,
}

impl<'info> ReadClockFullFromAccount<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = self.clock;
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
