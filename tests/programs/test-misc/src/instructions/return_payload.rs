use {
    crate::state::{ReturnPayload, TestMiscProgram, RETURN_PAYLOAD_VALUE},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct ReturnPayloadInstruction {
    pub program: Program<TestMiscProgram>,
}

impl ReturnPayloadInstruction {
    #[inline(always)]
    pub fn handler(&self) -> Result<ReturnPayload, ProgramError> {
        Ok(RETURN_PAYLOAD_VALUE)
    }
}
