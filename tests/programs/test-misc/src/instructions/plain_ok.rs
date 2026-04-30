use {crate::state::TestMiscProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct PlainOk {
    pub program: Program<TestMiscProgram>,
}

impl PlainOk {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
