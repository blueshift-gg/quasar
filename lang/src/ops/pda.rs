//! PDA check op: Phase 3 address verification against a derived PDA.
//!
//! `pda::Check` runs in `after_load` (Phase 3a) to verify that the loaded
//! field's address matches the expected PDA computed in Phase 1.

use {
    super::{AccountOp, OpCtx},
    crate::traits::AsAccountView,
    solana_program_error::ProgramError,
};

/// PDA address check. The derive macro computes seeds and the expected
/// address in Phase 1, then emits a `pda::Check::after_load` call in Phase 3.
pub struct Check<'a> {
    pub expected: &'a solana_address::Address,
    pub bump_out: &'a mut u8,
}

impl<'a, F: AsAccountView> AccountOp<F> for Check<'a> {
    #[inline(always)]
    fn after_load(&self, field: &F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        if !crate::keys_eq(field.to_account_view().address(), self.expected) {
            return Err(crate::error::QuasarError::InvalidPda.into());
        }
        Ok(())
    }
}
