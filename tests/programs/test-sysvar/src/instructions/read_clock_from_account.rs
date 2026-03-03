use quasar_core::prelude::*;
use quasar_core::sysvars::clock::Clock;

use crate::state::ClockSnapshot;

#[derive(Accounts)]
pub struct ReadClockFromAccount<'info> {
    pub _payer: &'info Signer,
    #[account(mut)]
    pub snapshot: &'info mut Account<ClockSnapshot>,
    pub clock: &'info Sysvar<Clock>,
}

impl<'info> ReadClockFromAccount<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = self.clock;
        self.snapshot.set(&ClockSnapshot {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        })
    }
}
