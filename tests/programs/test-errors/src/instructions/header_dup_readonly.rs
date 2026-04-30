use {quasar_derive::Accounts, quasar_lang::prelude::*};

/// Tests: duplicate readonly aliases are accepted when explicitly annotated.
#[derive(Accounts)]
pub struct HeaderDupReadonly {
    pub source: Signer,
    /// CHECK: test-only — validates that duplicate readonly aliases are parsed
    /// correctly.
    #[account(dup)]
    pub destination: UncheckedAccount,
}

impl HeaderDupReadonly {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
