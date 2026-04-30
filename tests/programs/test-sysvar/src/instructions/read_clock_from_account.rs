use {
    crate::state::{ClockSnapshot, ClockSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{prelude::*, sysvars::clock::Clock},
};

#[derive(Accounts)]
pub struct ReadClockFromAccount {
    pub _payer: Signer,
    #[account(mut)]
    pub snapshot: Account<ClockSnapshot>,
    pub clock: Sysvar<Clock>,
}

impl ReadClockFromAccount {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = &self.clock;
        self.snapshot.set_inner(ClockSnapshotInner {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        });
        Ok(())
    }
}
