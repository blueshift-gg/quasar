use {
    crate::state::RentCalcSnapshot,
    quasar_core::{
        prelude::*,
        sysvars::{rent::Rent, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadRentCalc<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"rent_calc"], bump)]
    pub snapshot: &'info mut Account<RentCalcSnapshot>,
    pub system_program: &'info Program<System>,
}

impl<'info> ReadRentCalc<'info> {
    #[inline(always)]
    pub fn handler(&mut self, data_len: u64) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(data_len as usize);
        self.snapshot.set_inner(min_balance);
        Ok(())
    }
}
