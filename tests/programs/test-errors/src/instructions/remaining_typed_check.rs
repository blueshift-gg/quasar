// Typed remaining-accounts parsing: unlike the raw `iter()` fixture in
// test-misc, `parse::<Account<T>, N>` runs duplicate rejection plus the full
// owner/discriminator/length account load on every remaining entry, so the
// suite can drive those rejection paths from hostile account lists.
use {
    crate::state::ErrorTestAccount,
    quasar_derive::Accounts,
    quasar_lang::{accounts::Account, prelude::*, remaining::RemainingAccounts},
};

#[derive(Accounts)]
pub struct RemainingTypedCheck {
    pub authority: Signer,
}

impl RemainingTypedCheck {
    #[inline(always)]
    pub fn handler(&self, remaining: RemainingAccounts<'_>) -> Result<(), ProgramError> {
        let parsed = remaining.parse::<Account<ErrorTestAccount>, 4>()?;
        if core::hint::black_box(parsed.iter().count()) > 4 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}
