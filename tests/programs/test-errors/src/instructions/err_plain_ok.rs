// Callee that succeeds without setting return data, so a caller using
// `invoke_with_return` on it exercises the MissingReturnData path.
use {crate::state::TestErrorsProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ErrPlainOk {
    pub program: Program<TestErrorsProgram>,
}

impl ErrPlainOk {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
