use {crate::events::SimpleEvent, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct EmitU64Event<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitU64Event<'info> {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value });
        Ok(())
    }
}
