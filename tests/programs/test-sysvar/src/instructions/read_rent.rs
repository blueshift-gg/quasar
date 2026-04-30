use {
    crate::state::{RentSnapshot, RentSnapshotInner},
    quasar_derive::Accounts,
    quasar_lang::{
        prelude::*,
        sysvars::{rent::Rent, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadRent {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = RentSnapshot::seeds())]
    pub snapshot: Account<RentSnapshot>,
    pub system_program: Program<SystemProgram>,
}

impl ReadRent {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(100);
        self.snapshot.set_inner(RentSnapshotInner {
            min_balance_100: min_balance,
        });
        Ok(())
    }
}
