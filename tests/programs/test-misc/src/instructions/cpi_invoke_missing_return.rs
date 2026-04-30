use {crate::state::TestMiscProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiInvokeMissingReturn {
    pub program: Program<TestMiscProgram>,
}

impl CpiInvokeMissingReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [45],
        )
        .invoke_with_return()?;

        match quasar_lang::cpi::CpiCall::<1, 1>::new(
            &crate::ID,
            [quasar_lang::cpi::InstructionAccount::readonly(
                self.program.address(),
            )],
            [self.program.to_account_view()],
            [47],
        )
        .invoke_with_return()
        {
            Err(err) if err == QuasarError::MissingReturnData.into() => Ok(()),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
