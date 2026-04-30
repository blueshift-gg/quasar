use {
    crate::state::{ClockSnapshot, ClockSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{
        prelude::*,
        sysvars::{clock::Clock, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadClock {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = ClockSnapshot::seeds())]
    pub snapshot: Account<ClockSnapshot>,
    pub system_program: Program<SystemProgram>,
}

impl ReadClock {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        self.snapshot.set_inner(ClockSnapshotInner {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        });
        Ok(())
    }
}
