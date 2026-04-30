use {
    crate::state::{ClockFullSnapshot, ClockFullSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{prelude::*, sysvars::clock::Clock},
};

#[derive(Accounts)]
pub struct ReadClockFullFromAccount {
    pub _payer: Signer,
    #[account(mut)]
    pub snapshot: Account<ClockFullSnapshot>,
    pub clock: Sysvar<Clock>,
}

impl ReadClockFullFromAccount {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = &self.clock;
        self.snapshot.set_inner(ClockFullSnapshotInner {
            slot: clock.slot.get(),
            epoch_start_timestamp: clock.epoch_start_timestamp.get(),
            epoch: clock.epoch.get(),
            leader_schedule_epoch: clock.leader_schedule_epoch.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        });
        Ok(())
    }
}
