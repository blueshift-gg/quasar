use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: "Account 'account' (index 0): must be writable signer with no
/// duplicates"
#[derive(Accounts)]
pub struct HeaderNoDupMutSigner {
    #[account(mut)]
    pub account: Signer,
}

impl HeaderNoDupMutSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
