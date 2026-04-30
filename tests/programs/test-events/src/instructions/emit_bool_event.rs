use {crate::events::BoolEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitBoolEvent {
    pub signer: Signer,
}

impl EmitBoolEvent {
    #[inline(always)]
    pub fn handler(&self, flag: bool) -> Result<(), ProgramError> {
        emit!(BoolEvent { flag });
        Ok(())
    }
}
