use quasar_core::prelude::*;

use crate::events::MultiEvent;

#[derive(Accounts)]
pub struct EmitMultiField<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitMultiField<'info> {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64, c: Address) -> Result<(), ProgramError> {
        emit!(MultiEvent { a, b, c });
        Ok(())
    }
}
