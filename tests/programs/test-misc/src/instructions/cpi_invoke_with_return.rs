use {
    crate::state::{TestMiscProgram, RETURN_U64_VALUE},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct CpiInvokeWithReturn {
    pub program: Program<TestMiscProgram>,
}

impl CpiInvokeWithReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [45],
        )
        .invoke_with_return()?;

        let expected = <u64 as InstructionArg>::to_zc(&RETURN_U64_VALUE);
        let expected_bytes = unsafe {
            core::slice::from_raw_parts(
                &expected as *const <u64 as InstructionArg>::Zc as *const u8,
                core::mem::size_of::<<u64 as InstructionArg>::Zc>(),
            )
        };

        if ret.as_slice() != expected_bytes {
            return Err(ProgramError::InvalidInstructionData);
        }
        if ret.decode::<u64>()? != RETURN_U64_VALUE {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(())
    }
}
