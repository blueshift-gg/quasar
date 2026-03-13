use {crate::events::EmptyEvent, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct EmitEmptyEvent<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitEmptyEvent<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        emit!(EmptyEvent {});
        Ok(())
    }
}
