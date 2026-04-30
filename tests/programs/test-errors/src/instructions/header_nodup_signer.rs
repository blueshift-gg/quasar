use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'account' (index 0): must be signer with no duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupSigner {
    pub account: Signer,
}

impl HeaderNoDupSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
