// Callee returning a 12-byte payload; callers decoding it as `u64` (8 bytes)
// exercise the InvalidReturnData length check.
use {
    crate::state::{ReturnPair, TestErrorsProgram, RETURN_PAIR_VALUE},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct ReturnPairInstruction {
    pub program: Program<TestErrorsProgram>,
}

impl ReturnPairInstruction {
    #[inline(always)]
    pub fn handler(&self) -> Result<ReturnPair, ProgramError> {
        Ok(RETURN_PAIR_VALUE)
    }
}
