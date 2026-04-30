use {crate::events::AddressEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitAddressEvent {
    pub signer: Signer,
}

impl EmitAddressEvent {
    #[inline(always)]
    pub fn handler(&self, addr: Address, value: u64) -> Result<(), ProgramError> {
        emit!(AddressEvent { addr, value });
        Ok(())
    }
}
