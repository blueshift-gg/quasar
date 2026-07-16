// Outer hop for the ReturnDataFromWrongProgram construction: self-CPIs
// cpi_nested_misc_return (disc 36) with invoke_with_return. The middle hop
// leaves return data set by test-misc, so the program-id check on the
// returned data must surface QuasarError::ReturnDataFromWrongProgram (3018).
use {crate::state::TestErrorsProgram, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CpiWrongReturnProgram {
    pub program: Program<TestErrorsProgram>,
    /// CHECK: fixture pass-through of the foreign return-data program.
    pub misc_program: UncheckedAccount,
}

impl CpiWrongReturnProgram {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = quasar_lang::cpi::CpiCall::<2, 1>::new(
            &crate::ID,
            [
                quasar_lang::cpi::InstructionAccount::readonly(self.program.address()),
                quasar_lang::cpi::InstructionAccount::readonly(self.misc_program.address()),
            ],
            [
                self.program.to_account_view(),
                self.misc_program.to_account_view(),
            ],
            [36],
        )
        .invoke_with_return()?;
        let _ = core::hint::black_box(ret.as_slice().len());
        Ok(())
    }
}
