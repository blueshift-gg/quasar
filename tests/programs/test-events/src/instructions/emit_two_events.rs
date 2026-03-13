use {
    crate::events::{SecondSimpleEvent, SimpleEvent},
    quasar_core::prelude::*,
};

#[derive(Accounts)]
pub struct EmitTwoEvents<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitTwoEvents<'info> {
    #[inline(always)]
    pub fn handler(&self, first: u64, second: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value: first });
        emit!(SecondSimpleEvent { value: second });
        Ok(())
    }
}
