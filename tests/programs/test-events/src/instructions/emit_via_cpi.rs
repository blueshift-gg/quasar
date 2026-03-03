use quasar_core::prelude::*;

use crate::events::SimpleEvent;
use crate::EventAuthority;
use crate::QuasarTestEventsProgram;

#[derive(Accounts)]
pub struct EmitViaCpi<'info> {
    pub signer: &'info Signer,
    pub event_authority: &'info EventAuthority,
    pub program: &'info QuasarTestEventsProgram,
}

impl<'info> EmitViaCpi<'info> {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit_cpi!(SimpleEvent { value })?;
        Ok(())
    }
}
