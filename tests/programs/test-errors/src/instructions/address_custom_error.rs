use quasar_core::prelude::*;

use crate::errors::TestError;
use crate::state::ErrorTestAccount;

pub const EXPECTED_ADDR: Address = Address::new_from_array([99u8; 32]);

#[derive(Accounts)]
pub struct AddressCustomError<'info> {
    #[account(address = EXPECTED_ADDR @ TestError::AddressCustom)]
    pub target: &'info Account<ErrorTestAccount>,
}

impl<'info> AddressCustomError<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
