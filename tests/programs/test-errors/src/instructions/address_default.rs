use quasar_core::prelude::*;

use crate::state::ErrorTestAccount;

pub const EXPECTED_ADDR_DEFAULT: Address = Address::new_from_array([88u8; 32]);

#[derive(Accounts)]
pub struct AddressDefault<'info> {
    #[account(address = EXPECTED_ADDR_DEFAULT)]
    pub target: &'info Account<ErrorTestAccount>,
}

impl<'info> AddressDefault<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
