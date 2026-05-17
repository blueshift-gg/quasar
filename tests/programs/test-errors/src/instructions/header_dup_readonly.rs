use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: duplicate readonly aliases are accepted when explicitly annotated.
#[derive(Accounts)]
pub struct HeaderDupReadonly {
    pub source: Signer,
    /// Test-only unchecked account used to validate duplicate readonly aliases.
    #[account(dup)]
    pub destination: UncheckedAccount,
}

impl HeaderDupReadonly {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
