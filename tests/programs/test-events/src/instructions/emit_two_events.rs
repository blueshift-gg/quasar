use {
    crate::events::{SecondSimpleEvent, SimpleEvent},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct EmitTwoEvents {
    pub signer: Signer,
}

impl EmitTwoEvents {
    #[inline(always)]
    pub fn handler(&self, first: u64, second: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value: first });
        emit!(SecondSimpleEvent { value: second });
        Ok(())
    }
}
