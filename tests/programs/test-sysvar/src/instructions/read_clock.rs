use quasar_core::prelude::*;
use quasar_core::sysvars::clock::Clock;
use quasar_core::sysvars::Sysvar as _;

use crate::state::ClockSnapshot;

#[derive(Accounts)]
pub struct ReadClock<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"clock"], bump)]
    pub snapshot: &'info mut Account<ClockSnapshot>,
    pub system_program: &'info Program<System>,
}

impl<'info> ReadClock<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        self.snapshot.set(&ClockSnapshot {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        })
    }
}
