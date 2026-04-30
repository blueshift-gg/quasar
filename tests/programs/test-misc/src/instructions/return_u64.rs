use {
    crate::state::{TestMiscProgram, RETURN_U64_VALUE},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct ReturnU64 {
    pub program: Program<TestMiscProgram>,
}

impl ReturnU64 {
    #[inline(always)]
    pub fn handler(&self) -> Result<u64, ProgramError> {
        Ok(RETURN_U64_VALUE)
    }
}
