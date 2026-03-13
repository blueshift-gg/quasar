use {crate::events::BoolEvent, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct EmitBoolEvent<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitBoolEvent<'info> {
    #[inline(always)]
    pub fn handler(&self, flag: bool) -> Result<(), ProgramError> {
        emit!(BoolEvent { flag });
        Ok(())
    }
}
