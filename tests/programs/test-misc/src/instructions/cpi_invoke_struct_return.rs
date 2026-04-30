use {
    crate::state::{ReturnPayload, TestMiscProgram, RETURN_PAYLOAD_VALUE},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CpiInvokeStructReturn {
    pub program: Program<TestMiscProgram>,
}

impl CpiInvokeStructReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [46],
        )
        .invoke_with_return()?;

        let expected = <ReturnPayload as InstructionArg>::to_zc(&RETURN_PAYLOAD_VALUE);
        let expected_bytes = unsafe {
            core::slice::from_raw_parts(
                &expected as *const <ReturnPayload as InstructionArg>::Zc as *const u8,
                core::mem::size_of::<<ReturnPayload as InstructionArg>::Zc>(),
            )
        };

        if ret.as_slice() != expected_bytes {
            return Err(ProgramError::InvalidInstructionData);
        }
        if ret.decode::<ReturnPayload>()? != RETURN_PAYLOAD_VALUE {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(())
    }
}
