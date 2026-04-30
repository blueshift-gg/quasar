use {crate::events::MultiEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitMultiField {
    pub signer: Signer,
}

impl EmitMultiField {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64, c: Address) -> Result<(), ProgramError> {
        emit!(MultiEvent { a, b, c });
        Ok(())
    }
}
