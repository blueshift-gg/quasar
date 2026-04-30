use {
    crate::state::{ClockFullSnapshot, ClockFullSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{
        prelude::*,
        sysvars::{clock::Clock, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadClockFull {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = ClockFullSnapshot::seeds())]
    pub snapshot: Account<ClockFullSnapshot>,
    pub system_program: Program<SystemProgram>,
}

impl ReadClockFull {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
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
