use {crate::events::EmptyEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitEmptyEvent {
    pub signer: Signer,
}

impl EmitEmptyEvent {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        emit!(EmptyEvent {});
        Ok(())
    }
}
