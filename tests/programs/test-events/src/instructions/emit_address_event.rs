use quasar_core::prelude::*;

use crate::events::AddressEvent;

#[derive(Accounts)]
pub struct EmitAddressEvent<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitAddressEvent<'info> {
    #[inline(always)]
    pub fn handler(&self, addr: Address, value: u64) -> Result<(), ProgramError> {
        emit!(AddressEvent { addr, value });
        Ok(())
    }
}
