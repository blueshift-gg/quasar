use {
    crate::state::RentSnapshot,
    quasar_core::{
        prelude::*,
        sysvars::{rent::Rent, Sysvar as _},
    },
};

#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"rent"], bump)]
    pub snapshot: &'info mut Account<RentSnapshot>,
    pub system_program: &'info Program<System>,
}

impl<'info> ReadRent<'info> {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(100);
        self.snapshot.set_inner(min_balance);
        Ok(())
    }
}
