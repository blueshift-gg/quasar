use quasar_core::prelude::*;
use quasar_core::sysvars::rent::Rent;
use quasar_core::sysvars::Sysvar as _;

use crate::state::{RentSnapshot, RentSnapshotInit};

#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"rent"], bump)]
    pub snapshot: &'info mut Account<RentSnapshot>,
    pub system_program: &'info SystemProgram,
}

impl<'info> ReadRent<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(100);
        self.snapshot.set(&RentSnapshotInit {
            min_balance_100: min_balance,
        })
    }
}
