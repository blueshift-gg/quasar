use {crate::state::ErrorTestAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

pub const EXPECTED_ADDR_DEFAULT: Address = Address::new_from_array([88u8; 32]);

#[derive(Accounts)]
pub struct AddressDefault {
    #[account(address = EXPECTED_ADDR_DEFAULT)]
    pub target: Account<ErrorTestAccount>,
}

impl AddressDefault {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
