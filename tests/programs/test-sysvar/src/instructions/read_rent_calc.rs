use {
    crate::state::{RentCalcSnapshot, RentCalcSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{
        prelude::*,
        sysvars::{rent::Rent, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadRentCalc {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = RentCalcSnapshot::seeds())]
    pub snapshot: Account<RentCalcSnapshot>,
    pub system_program: Program<SystemProgram>,
}

impl ReadRentCalc {
    #[inline(always)]
    pub fn handler(&mut self, data_len: u64) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(data_len as usize);
        self.snapshot
            .set_inner(RentCalcSnapshotInner { min_balance });
        Ok(())
    }
}
