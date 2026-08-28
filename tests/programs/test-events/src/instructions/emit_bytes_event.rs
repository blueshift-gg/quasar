use {crate::events::BytesEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitBytesEvent {
    pub signer: Signer,
}

impl EmitBytesEvent {
    #[inline(always)]
    pub fn handler(&self, hash: [u8; 32], amount: u64) -> Result<(), ProgramError> {
        emit!(BytesEvent { hash, amount });
        Ok(())
    }
}
