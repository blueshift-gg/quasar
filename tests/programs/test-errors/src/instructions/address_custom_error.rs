use {
    crate::{errors::TestError, state::ErrorTestAccount},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

pub const EXPECTED_ADDR: Address = Address::new_from_array([99u8; 32]);

#[derive(Accounts)]
pub struct AddressCustomError {
    #[account(address = EXPECTED_ADDR @ TestError::AddressCustom)]
    pub target: Account<ErrorTestAccount>,
}

impl AddressCustomError {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
