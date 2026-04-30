use {crate::events::SimpleEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitU64Event {
    pub signer: Signer,
}

impl EmitU64Event {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value });
        Ok(())
    }
}
